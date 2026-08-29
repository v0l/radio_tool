//! Bluetooth Low Energy as a [`ByteStream`].
//!
//! Radios that clone over Bluetooth expose what is effectively a serial port:
//! one characteristic to write to and one that notifies back. The UV-17Pro
//! family speaks the same protocol over this as over a cable, so the driver
//! does not change, only the transport.
//!
//! Finding the right pair of characteristics is the fiddly part. A radio
//! advertises several services that look writable and notifiable, so picking
//! the first match finds the wrong one. The services known to carry a serial
//! link are tried in order first, and only then anything that looks usable.

use crate::{ByteStream, Error, Result};
use btleplug::api::{
    Central, CharPropFlags, Characteristic, Manager as _, Peripheral as _, ScanFilter, WriteType,
};
use btleplug::platform::{Adapter, Manager, Peripheral};
use futures::StreamExt;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use uuid::Uuid;

/// Services that carry a serial style link, in the order to prefer them
const SERIAL_SERVICES: [Uuid; 3] = [
    // the common HM-10 style module
    Uuid::from_u128(0x0000ffe0_0000_1000_8000_00805f9b34fb),
    Uuid::from_u128(0x0000fff0_0000_1000_8000_00805f9b34fb),
    // Nordic UART
    Uuid::from_u128(0x6e400001_b5a3_f393_e0a9_e50e24dcca9e),
];

/// A radio seen while scanning
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Device {
    /// Bluetooth address, as the platform reports it
    pub address: String,
    /// Advertised name, when it has told us one
    pub name: String,
}

/// Everything that has to survive for the connection to stay up
pub struct BleStream {
    runtime: tokio::runtime::Runtime,
    peripheral: Peripheral,
    write: Characteristic,
    write_type: WriteType,
    notify: Characteristic,
    /// Filled by the notification task, drained by reads
    incoming: Arc<Mutex<VecDeque<u8>>>,
    timeout: Duration,
    /// Largest write the link will carry
    chunk: usize,
}

impl std::fmt::Debug for BleStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BleStream")
            .field("write", &self.write.uuid)
            .field("notify", &self.notify.uuid)
            .field(
                "with_response",
                &matches!(self.write_type, WriteType::WithResponse),
            )
            .finish()
    }
}

fn runtime() -> Result<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| Error::Port(format!("cannot start the Bluetooth runtime: {e}")))
}

async fn adapter(which: Option<&str>) -> Result<Adapter> {
    let manager = Manager::new()
        .await
        .map_err(|e| Error::Port(format!("Bluetooth unavailable: {e}")))?;
    let adapters = manager
        .adapters()
        .await
        .map_err(|e| Error::Port(format!("cannot list Bluetooth adapters: {e}")))?;

    if adapters.is_empty() {
        return Err(Error::Port("no Bluetooth adapter found".to_owned()));
    }

    match which {
        None => adapters
            .into_iter()
            .next()
            .ok_or_else(|| Error::Port("no Bluetooth adapter found".to_owned())),
        Some(wanted) => {
            for a in &adapters {
                if let Ok(info) = a.adapter_info().await {
                    if info.to_lowercase().contains(&wanted.to_lowercase()) {
                        return Ok(a.clone());
                    }
                }
            }
            Err(Error::Port(format!(
                "no Bluetooth adapter matching {wanted:?}"
            )))
        }
    }
}

/// Scan for devices that are advertising a name
pub fn scan(seconds: u64, which_adapter: Option<&str>) -> Result<Vec<Device>> {
    let rt = runtime()?;
    rt.block_on(async {
        let adapter = adapter(which_adapter).await?;
        adapter
            .start_scan(ScanFilter::default())
            .await
            .map_err(|e| Error::Port(format!("cannot scan: {e}")))?;

        // a radio often only gives its name in a later advertisement, so keep
        // looking for the whole window rather than taking the first sighting
        let deadline = Instant::now() + Duration::from_secs(seconds);
        let mut seen: std::collections::BTreeMap<String, String> = Default::default();

        while Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(500)).await;
            for p in adapter.peripherals().await.unwrap_or_default() {
                let address = p.address().to_string();
                let name = p
                    .properties()
                    .await
                    .ok()
                    .flatten()
                    .and_then(|props| props.local_name)
                    .unwrap_or_default();
                let entry = seen.entry(address).or_default();
                if entry.is_empty() && !name.is_empty() {
                    *entry = name;
                }
            }
        }

        let _ = adapter.stop_scan().await;
        Ok(seen
            .into_iter()
            .map(|(address, name)| Device { address, name })
            .collect())
    })
}

impl BleStream {
    /// Connect to a radio by Bluetooth address and find its serial service
    pub fn connect(
        address: &str,
        which_adapter: Option<&str>,
        scan_seconds: u64,
        timeout: Duration,
    ) -> Result<Self> {
        let rt = runtime()?;
        let wanted = address.to_lowercase();

        let (peripheral, write, write_type, notify, chunk, incoming) = rt.block_on(async {
            let adapter = adapter(which_adapter).await?;
            adapter
                .start_scan(ScanFilter::default())
                .await
                .map_err(|e| Error::Port(format!("cannot scan: {e}")))?;

            let deadline = Instant::now() + Duration::from_secs(scan_seconds);
            let mut found: Option<Peripheral> = None;
            while Instant::now() < deadline && found.is_none() {
                tokio::time::sleep(Duration::from_millis(300)).await;
                for p in adapter.peripherals().await.unwrap_or_default() {
                    if p.address().to_string().to_lowercase() == wanted {
                        found = Some(p);
                        break;
                    }
                }
            }
            let _ = adapter.stop_scan().await;

            let peripheral = found.ok_or_else(|| {
                Error::Port(format!(
                    "no Bluetooth device at {address}, is the radio on with Bluetooth enabled?"
                ))
            })?;

            peripheral
                .connect()
                .await
                .map_err(|e| Error::Port(format!("cannot connect to {address}: {e}")))?;
            peripheral
                .discover_services()
                .await
                .map_err(|e| Error::Port(format!("cannot discover services: {e}")))?;

            let characteristics: Vec<Characteristic> =
                peripheral.characteristics().into_iter().collect();

            let (write, notify) = pick_serial_pair(&characteristics).ok_or_else(|| {
                Error::Port(
                    "this device has no serial service: nothing to write to and be notified on"
                        .to_owned(),
                )
            })?;

            // a write cannot exceed the negotiated MTU
            let chunk = 20usize;

            let write_type = if write
                .properties
                .contains(CharPropFlags::WRITE_WITHOUT_RESPONSE)
            {
                WriteType::WithoutResponse
            } else {
                WriteType::WithResponse
            };

            peripheral
                .subscribe(&notify)
                .await
                .map_err(|e| Error::Port(format!("cannot subscribe to notifications: {e}")))?;

            let incoming: Arc<Mutex<VecDeque<u8>>> = Arc::new(Mutex::new(VecDeque::new()));
            let sink = Arc::clone(&incoming);
            let mut notifications = peripheral
                .notifications()
                .await
                .map_err(|e| Error::Port(format!("cannot read notifications: {e}")))?;

            tokio::spawn(async move {
                while let Some(data) = notifications.next().await {
                    if let Ok(mut buf) = sink.lock() {
                        buf.extend(data.value);
                    }
                }
            });

            Ok::<_, Error>((peripheral, write, write_type, notify, chunk, incoming))
        })?;

        Ok(Self {
            runtime: rt,
            peripheral,
            write,
            write_type,
            notify,
            incoming,
            timeout,
            chunk,
        })
    }

    /// The block size this link works best with. A packet based link carries
    /// larger clone blocks than a cable does.
    pub fn block_size_hint(&self) -> usize {
        0x80
    }
}

/// Find a characteristic to write to and one to be notified on, both from the
/// same service, preferring the services known to carry a serial link
fn pick_serial_pair(
    characteristics: &[Characteristic],
) -> Option<(Characteristic, Characteristic)> {
    let examine = |service: Uuid| -> Option<(Characteristic, Characteristic)> {
        let in_service: Vec<&Characteristic> = characteristics
            .iter()
            .filter(|c| c.service_uuid == service)
            .collect();

        let notify = in_service
            .iter()
            .find(|c| c.properties.contains(CharPropFlags::NOTIFY))?;

        // prefer a characteristic that also notifies: these modules are often
        // one characteristic behaving like a serial port, and a radio answers
        // on the one it was spoken to on
        let writable = |c: &Characteristic| {
            c.properties.contains(CharPropFlags::WRITE)
                || c.properties.contains(CharPropFlags::WRITE_WITHOUT_RESPONSE)
        };
        let write = in_service
            .iter()
            .find(|c| writable(c) && c.properties.contains(CharPropFlags::NOTIFY))
            .or_else(|| in_service.iter().find(|c| writable(c)))?;

        Some(((*write).clone(), (*notify).clone()))
    };

    for known in SERIAL_SERVICES {
        if let Some(pair) = examine(known) {
            return Some(pair);
        }
    }

    // nothing recognisable, take the first service that could work
    let mut services: Vec<Uuid> = characteristics.iter().map(|c| c.service_uuid).collect();
    services.sort();
    services.dedup();
    services.into_iter().find_map(examine)
}

impl ByteStream for BleStream {
    fn write_all(&mut self, data: &[u8]) -> Result<()> {
        let peripheral = self.peripheral.clone();
        let characteristic = self.write.clone();
        let write_type = self.write_type;
        let chunk = self.chunk;
        let data = data.to_vec();

        self.runtime.block_on(async move {
            for piece in data.chunks(chunk) {
                peripheral
                    .write(&characteristic, piece, write_type)
                    .await
                    .map_err(|e| Error::Port(format!("write failed: {e}")))?;
            }
            Ok(())
        })
    }

    fn read(&mut self, len: usize) -> Result<Vec<u8>> {
        let deadline = Instant::now() + self.timeout;

        loop {
            {
                let Ok(mut buf) = self.incoming.lock() else {
                    return Err(Error::Port("the notification task died".to_owned()));
                };
                if buf.len() >= len {
                    return Ok(buf.drain(..len).collect());
                }
                if Instant::now() >= deadline {
                    // hand back whatever arrived, the caller decides if it is
                    // enough
                    return Ok(buf.drain(..).collect());
                }
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn flush_input(&mut self) -> Result<()> {
        if let Ok(mut buf) = self.incoming.lock() {
            buf.clear();
        }
        Ok(())
    }

    fn sleep(&mut self, millis: u64) {
        std::thread::sleep(Duration::from_millis(millis));
    }
}

impl Drop for BleStream {
    fn drop(&mut self) {
        let peripheral = self.peripheral.clone();
        let notify = self.notify.clone();
        // best effort: nothing useful to do while tearing down
        self.runtime.block_on(async move {
            let _ = peripheral.unsubscribe(&notify).await;
            let _ = peripheral.disconnect().await;
        });
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn characteristic(service: Uuid, uuid: Uuid, properties: CharPropFlags) -> Characteristic {
        Characteristic {
            uuid,
            service_uuid: service,
            properties,
            descriptors: Default::default(),
        }
    }

    #[test]
    fn a_known_serial_service_is_preferred_over_an_earlier_one() {
        let other = Uuid::from_u128(0x0000180a_0000_1000_8000_00805f9b34fb);
        let hm10 = SERIAL_SERVICES[0];

        let chars = vec![
            // a device information service that also looks writable
            characteristic(
                other,
                Uuid::from_u128(1),
                CharPropFlags::WRITE | CharPropFlags::NOTIFY,
            ),
            characteristic(
                hm10,
                Uuid::from_u128(2),
                CharPropFlags::WRITE_WITHOUT_RESPONSE | CharPropFlags::NOTIFY,
            ),
        ];

        let (write, notify) = pick_serial_pair(&chars).expect("a pair is found");
        assert_eq!(write.service_uuid, hm10, "the serial service must win");
        assert_eq!(notify.service_uuid, hm10);
    }

    #[test]
    fn a_characteristic_that_both_writes_and_notifies_is_preferred() {
        let hm10 = SERIAL_SERVICES[0];
        let chars = vec![
            characteristic(hm10, Uuid::from_u128(1), CharPropFlags::WRITE),
            characteristic(
                hm10,
                Uuid::from_u128(2),
                CharPropFlags::WRITE | CharPropFlags::NOTIFY,
            ),
        ];

        let (write, _) = pick_serial_pair(&chars).expect("a pair is found");
        assert_eq!(
            write.uuid,
            Uuid::from_u128(2),
            "the radio answers on the characteristic it was spoken to on"
        );
    }

    #[test]
    fn an_unknown_service_is_used_only_as_a_fallback() {
        let odd = Uuid::from_u128(0x12345678_0000_1000_8000_00805f9b34fb);
        let chars = vec![characteristic(
            odd,
            Uuid::from_u128(1),
            CharPropFlags::WRITE | CharPropFlags::NOTIFY,
        )];

        let (write, _) = pick_serial_pair(&chars).expect("the fallback finds it");
        assert_eq!(write.service_uuid, odd);
    }

    #[test]
    fn a_device_with_nothing_usable_is_refused() {
        let odd = Uuid::from_u128(0x12345678_0000_1000_8000_00805f9b34fb);

        // notify but nothing to write to
        let chars = vec![characteristic(
            odd,
            Uuid::from_u128(1),
            CharPropFlags::NOTIFY,
        )];
        assert!(pick_serial_pair(&chars).is_none());

        // write but no notification to read back on
        let chars = vec![characteristic(
            odd,
            Uuid::from_u128(1),
            CharPropFlags::WRITE,
        )];
        assert!(pick_serial_pair(&chars).is_none());

        assert!(pick_serial_pair(&[]).is_none());
    }

    #[test]
    fn the_pair_must_come_from_one_service() {
        let a = SERIAL_SERVICES[0];
        let b = SERIAL_SERVICES[1];
        // writable in one service, notifiable in another: not a serial link
        let chars = vec![
            characteristic(a, Uuid::from_u128(1), CharPropFlags::WRITE),
            characteristic(b, Uuid::from_u128(2), CharPropFlags::NOTIFY),
        ];
        assert!(pick_serial_pair(&chars).is_none());
    }
}
