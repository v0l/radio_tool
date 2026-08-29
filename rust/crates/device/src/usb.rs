//! Finding radios on USB, and talking to them.
//!
//! Two separate things live here, and the split is deliberate. Which radio a
//! USB device is, is a lookup in [`KNOWN`] and involves no hardware at all,
//! so it is tested. Opening the device and moving bytes is a thin wrapper
//! over `nusb` that cannot be tested without a radio plugged in, so it is
//! kept as small as possible and holds no decisions.

use crate::dfu::ControlTransfer;
use crate::{ByteStream, Error, Result};
use nusb::MaybeFuture;
use nusb::transfer::{ControlIn, ControlOut, ControlType, Recipient};
use std::time::Duration;

/// How a radio expects to be spoken to once it is open
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    /// USB DFU with the TYT vendor commands
    TytDfu,
    /// The TYT HID protocol, used by the radios with an SGL container
    TytHid,
    /// The H8SX boot protocol over bulk endpoints
    H8sx,
}

/// A radio that can be recognised on USB
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Known {
    /// USB vendor id
    pub vendor: u16,
    /// USB product id
    pub product: u16,
    /// What it speaks
    pub protocol: Protocol,
    /// What to call it before it has said what it is
    pub description: &'static str,
}

/// Every USB device this tool recognises.
///
/// The identifiers belong to the bootloader the radio exposes, not to the
/// radio, so several models share one entry and the model is only known once
/// the device has been asked. `0483:df11` in particular is the ST DFU
/// bootloader, which is on a great many devices that are not radios.
pub const KNOWN: &[Known] = &[
    Known {
        vendor: 0x0483,
        product: 0xdf11,
        protocol: Protocol::TytDfu,
        description: "TYT or Retevis radio in DFU mode",
    },
    Known {
        vendor: 0x15a2,
        product: 0x0073,
        protocol: Protocol::TytHid,
        description: "Radioddity or Baofeng radio in bootloader mode",
    },
    Known {
        vendor: 0x045b,
        product: 0x0025,
        protocol: Protocol::H8sx,
        description: "Yaesu radio in bootloader mode",
    },
];

/// Work out whether a USB device is a radio this tool knows
pub fn identify(vendor: u16, product: u16) -> Option<&'static Known> {
    KNOWN
        .iter()
        .find(|k| k.vendor == vendor && k.product == product)
}

/// A radio found on the bus
#[derive(Debug, Clone)]
pub struct Found {
    /// What it is
    pub known: &'static Known,
    /// Manufacturer string, if the device offers one
    pub manufacturer: Option<String>,
    /// Product string, if the device offers one
    pub product_name: Option<String>,
    /// Bus and address, to tell two of the same radio apart
    pub bus: String,
    /// Device address on that bus
    pub address: u8,
}

impl std::fmt::Display for Found {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:04x}:{:04x}  {}",
            self.known.vendor, self.known.product, self.known.description
        )?;
        if let Some(name) = &self.product_name {
            write!(f, " ({name})")?;
        }
        Ok(())
    }
}

/// List radios currently plugged in
pub fn list() -> Result<Vec<Found>> {
    let devices = nusb::list_devices()
        .wait()
        .map_err(|e| Error::Port(format!("cannot list USB devices: {e}")))?;

    Ok(devices
        .filter_map(|info| {
            let known = identify(info.vendor_id(), info.product_id())?;
            Some(Found {
                known,
                manufacturer: info.manufacturer_string().map(str::to_owned),
                product_name: info.product_string().map(str::to_owned),
                bus: info.bus_id().to_owned(),
                address: info.device_address(),
            })
        })
        .collect())
}

/// An open USB device.
///
/// This holds no protocol knowledge. It answers control transfers for
/// [`ControlTransfer`], and bulk transfers for [`ByteStream`], so the DFU and
/// H8SX code above it does not change between a real radio and a fake.
pub struct UsbDevice {
    device: nusb::Device,
    interface: Option<nusb::Interface>,
    timeout: Duration,
    /// Endpoints for the protocols that use bulk transfers
    bulk_out: u8,
    bulk_in: u8,
}

impl std::fmt::Debug for UsbDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UsbDevice")
            .field("timeout", &self.timeout)
            .finish()
    }
}

impl UsbDevice {
    /// Open the first radio matching a vendor and product id
    pub fn open(vendor: u16, product: u16, timeout: Duration) -> Result<Self> {
        let info = nusb::list_devices()
            .wait()
            .map_err(|e| Error::Port(format!("cannot list USB devices: {e}")))?
            .find(|d| d.vendor_id() == vendor && d.product_id() == product)
            .ok_or_else(|| {
                Error::Port(format!(
                    "no USB device {vendor:04x}:{product:04x}, is the radio in bootloader mode?"
                ))
            })?;

        let device = info
            .open()
            .wait()
            .map_err(|e| Error::Port(format!("cannot open the radio: {e}")))?;

        Ok(Self {
            device,
            interface: None,
            timeout,
            bulk_out: 0x02,
            bulk_in: 0x81,
        })
    }

    /// Claim an interface, which bulk transfers need and control transfers
    /// do not on every platform
    pub fn claim(&mut self, number: u8) -> Result<()> {
        let interface = self
            .device
            .claim_interface(number)
            .wait()
            .map_err(|e| Error::Port(format!("cannot claim interface {number}: {e}")))?;
        self.interface = Some(interface);
        Ok(())
    }

    /// Ask the device to describe its own memory.
    ///
    /// An ST DFU device puts a layout string in each alternate setting. This
    /// is what should decide where firmware may be written, rather than a
    /// chip layout compiled in here, because a radio uses it to withhold the
    /// sectors its bootloader is in.
    pub fn memory_layout(&self) -> Result<Vec<crate::dfuse::Memory>> {
        let config = self
            .device
            .active_configuration()
            .map_err(|e| Error::Port(format!("cannot read the configuration: {e}")))?;

        let mut found = Vec::new();
        for interface in config.interface_alt_settings() {
            let Some(index) = interface.string_index() else {
                continue;
            };

            let Ok(text) = self
                .device
                .get_string_descriptor(index, 0x0409, self.timeout)
                .wait()
            else {
                continue;
            };

            if let Ok(memory) = crate::dfuse::parse(&text, interface.alternate_setting()) {
                found.push(memory);
            }
        }

        if found.is_empty() {
            return Err(Error::Unexpected {
                what: "memory layout",
                expected: "a device that describes its own memory".to_owned(),
                got: "no layout string on any alternate setting".to_owned(),
            });
        }

        Ok(found)
    }

    /// Take an HID radio away from the kernel and claim it.
    ///
    /// Linux binds these radios with its own HID driver, so it has to be
    /// asked to let go before the interface can be claimed. The endpoints are
    /// the ones this family uses: bulk out, interrupt in.
    pub fn claim_hid(&mut self) -> Result<()> {
        let interface = self
            .device
            .detach_and_claim_interface(0)
            .wait()
            .map_err(|e| {
                Error::Port(format!(
                    "cannot take the radio from the kernel HID driver: {e}"
                ))
            })?;

        self.interface = Some(interface);
        self.bulk_out = 0x02;
        self.bulk_in = 0x81;

        // HID SET_IDLE. Some bootloaders in this family stall it, which does
        // not matter because only the endpoints are used afterwards, but the
        // C++ sends it and this follows.
        let _ = self.control_out(0x0a, 0, &[]);
        Ok(())
    }

    /// Select which memory an alternate setting refers to
    pub fn select_alt(&mut self, interface: u8, alt: u8) -> Result<()> {
        let handle = self
            .device
            .claim_interface(interface)
            .wait()
            .map_err(|e| Error::Port(format!("cannot claim interface {interface}: {e}")))?;
        handle
            .set_alt_setting(alt)
            .wait()
            .map_err(|e| Error::Port(format!("cannot select alternate setting {alt}: {e}")))?;
        self.interface = Some(handle);
        Ok(())
    }

    /// Set which endpoints [`ByteStream`] uses
    pub fn set_endpoints(&mut self, out: u8, in_: u8) {
        self.bulk_out = out;
        self.bulk_in = in_;
    }
}

impl ControlTransfer for UsbDevice {
    fn control_out(&mut self, request: u8, value: u16, data: &[u8]) -> Result<()> {
        self.device
            .control_out(
                ControlOut {
                    control_type: ControlType::Class,
                    recipient: Recipient::Interface,
                    request,
                    value,
                    index: 0,
                    data,
                },
                self.timeout,
            )
            .wait()
            .map_err(|e| Error::Port(format!("control transfer out failed: {e}")))?;
        Ok(())
    }

    fn control_in(&mut self, request: u8, value: u16, length: usize) -> Result<Vec<u8>> {
        let length = u16::try_from(length)
            .map_err(|_| Error::Port("control transfer longer than USB allows".to_owned()))?;

        self.device
            .control_in(
                ControlIn {
                    control_type: ControlType::Class,
                    recipient: Recipient::Interface,
                    request,
                    value,
                    index: 0,
                    length,
                },
                self.timeout,
            )
            .wait()
            .map_err(|e| Error::Port(format!("control transfer in failed: {e}")))
    }
}

impl crate::hid::HidTransport for UsbDevice {
    fn write(&mut self, data: &[u8]) -> Result<()> {
        // the out endpoint on these radios is bulk, whatever the HID class
        // suggests, and an interrupt transfer to it is rejected outright
        <Self as ByteStream>::write_all(self, data)
    }

    fn read_packet(&mut self) -> Result<Vec<u8>> {
        let interface = self
            .interface
            .as_mut()
            .ok_or_else(|| Error::Port("claim the interface first".to_owned()))?;

        // and the in endpoint is interrupt, not bulk. The radio always sends
        // a full packet, so asking for fewer bytes risks an overflow.
        let mut endpoint = interface
            .endpoint::<nusb::transfer::Interrupt, nusb::transfer::In>(self.bulk_in)
            .map_err(|e| Error::Port(format!("no interrupt in endpoint: {e}")))?;

        let buffer = endpoint.allocate(HID_PACKET);
        endpoint.submit(buffer);

        let completion = endpoint
            .wait_next_complete(self.timeout)
            .ok_or_else(|| Error::Port("the radio did not answer".to_owned()))?;
        completion
            .status
            .map_err(|e| Error::Port(format!("interrupt read failed: {e}")))?;

        Ok(completion.buffer.into_vec())
    }
}

/// A radio in this family always answers with a packet this size
const HID_PACKET: usize = 0x40;

impl ByteStream for UsbDevice {
    fn write_all(&mut self, data: &[u8]) -> Result<()> {
        let interface = self
            .interface
            .as_mut()
            .ok_or_else(|| Error::Port("claim an interface before bulk transfers".to_owned()))?;

        let mut endpoint = interface
            .endpoint::<nusb::transfer::Bulk, nusb::transfer::Out>(self.bulk_out)
            .map_err(|e| Error::Port(format!("no bulk out endpoint: {e}")))?;

        let mut buffer = endpoint.allocate(data.len());
        buffer.extend_from_slice(data);
        endpoint.submit(buffer);
        endpoint
            .wait_next_complete(self.timeout)
            .ok_or_else(|| Error::Port("bulk write timed out".to_owned()))?
            .status
            .map_err(|e| Error::Port(format!("bulk write failed: {e}")))?;
        Ok(())
    }

    fn read(&mut self, len: usize) -> Result<Vec<u8>> {
        let interface = self
            .interface
            .as_mut()
            .ok_or_else(|| Error::Port("claim an interface before bulk transfers".to_owned()))?;

        let mut endpoint = interface
            .endpoint::<nusb::transfer::Bulk, nusb::transfer::In>(self.bulk_in)
            .map_err(|e| Error::Port(format!("no bulk in endpoint: {e}")))?;

        let buffer = endpoint.allocate(len);
        endpoint.submit(buffer);

        let Some(completion) = endpoint.wait_next_complete(self.timeout) else {
            // a read that times out is not an error here, the protocols above
            // decide whether what arrived was enough
            return Ok(Vec::new());
        };
        completion
            .status
            .map_err(|e| Error::Port(format!("bulk read failed: {e}")))?;

        Ok(completion.buffer.into_vec())
    }

    fn flush_input(&mut self) -> Result<()> {
        // there is no buffer of our own to clear: anything the device has not
        // been asked for stays on the device
        Ok(())
    }

    fn sleep(&mut self, millis: u64) {
        std::thread::sleep(Duration::from_millis(millis));
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn known_radios_are_identified() {
        assert_eq!(
            identify(0x0483, 0xdf11).map(|k| k.protocol),
            Some(Protocol::TytDfu)
        );
        assert_eq!(
            identify(0x15a2, 0x0073).map(|k| k.protocol),
            Some(Protocol::TytHid)
        );
        assert_eq!(
            identify(0x045b, 0x0025).map(|k| k.protocol),
            Some(Protocol::H8sx)
        );
    }

    #[test]
    fn an_unknown_device_is_not_guessed_at() {
        // a mouse, a hub, and the same vendor with a different product
        assert!(identify(0x046d, 0xc077).is_none());
        assert!(identify(0x1d6b, 0x0002).is_none());
        assert!(identify(0x0483, 0x0001).is_none());
        assert!(identify(0, 0).is_none());
    }

    #[test]
    fn no_two_entries_claim_the_same_device() {
        for (i, a) in KNOWN.iter().enumerate() {
            for b in KNOWN.iter().skip(i + 1) {
                assert!(
                    a.vendor != b.vendor || a.product != b.product,
                    "{:04x}:{:04x} is listed twice",
                    a.vendor,
                    a.product
                );
            }
        }
    }

    #[test]
    fn every_entry_says_what_it_is() {
        for k in KNOWN {
            assert!(!k.description.is_empty());
            assert_ne!(k.vendor, 0);
        }
    }

    /// The identifiers belong to bootloaders, not to radios, and the ST one
    /// is on a great many devices that are not radios at all. Anything that
    /// reports what was found has to say so rather than promise a radio.
    #[test]
    fn the_st_bootloader_is_described_as_a_mode_not_as_a_certainty() {
        let st = identify(0x0483, 0xdf11).expect("known");
        assert!(
            st.description.contains("DFU mode"),
            "the description should say what mode was seen, not claim a model"
        );
    }
}
