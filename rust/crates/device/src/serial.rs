//! A serial port as a [`ByteStream`].

use crate::{ByteStream, Error, Result};
use std::io::{Read, Write};
use std::time::Duration;

/// A serial port, opened 8N1 at the radio's baud rate
pub struct SerialStream {
    port: Box<dyn serialport::SerialPort>,
    /// How long to wait for a read before giving up
    timeout: Duration,
}

impl std::fmt::Debug for SerialStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // the port itself has nothing worth printing and is not Debug
        f.debug_struct("SerialStream")
            .field("name", &self.port.name())
            .field("timeout", &self.timeout)
            .finish()
    }
}

impl SerialStream {
    /// Open a port. `baud` is the radio's rate, 9600 for the UV-5R family.
    pub fn open(path: &str, baud: u32, timeout: Duration) -> Result<Self> {
        let port = serialport::new(path, baud)
            .data_bits(serialport::DataBits::Eight)
            .parity(serialport::Parity::None)
            .stop_bits(serialport::StopBits::One)
            .flow_control(serialport::FlowControl::None)
            .timeout(timeout)
            .open()
            .map_err(|e| Error::Port(format!("cannot open {path}: {e}")))?;

        Ok(Self { port, timeout })
    }

    /// Ports this machine can see
    pub fn list() -> Vec<String> {
        serialport::available_ports()
            .unwrap_or_default()
            .into_iter()
            .map(|p| p.port_name)
            .collect()
    }
}

impl ByteStream for SerialStream {
    fn write_all(&mut self, data: &[u8]) -> Result<()> {
        self.port
            .write_all(data)
            .map_err(|e| Error::Port(format!("write failed: {e}")))?;
        self.port
            .flush()
            .map_err(|e| Error::Port(format!("flush failed: {e}")))
    }

    fn read(&mut self, len: usize) -> Result<Vec<u8>> {
        let mut out = Vec::with_capacity(len);
        let deadline = std::time::Instant::now() + self.timeout;

        // a serial read returns whatever has arrived, which is rarely the
        // whole block, so keep asking until the deadline rather than treating
        // a short read as the end
        while out.len() < len && std::time::Instant::now() < deadline {
            let mut buf = vec![0u8; len - out.len()];
            match self.port.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => out.extend_from_slice(buf.get(..n).unwrap_or_default()),
                Err(e) if e.kind() == std::io::ErrorKind::TimedOut => break,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(Error::Port(format!("read failed: {e}"))),
            }
        }
        Ok(out)
    }

    fn flush_input(&mut self) -> Result<()> {
        self.port
            .clear(serialport::ClearBuffer::All)
            .map_err(|e| Error::Port(format!("cannot flush: {e}")))
    }

    fn sleep(&mut self, millis: u64) {
        std::thread::sleep(Duration::from_millis(millis));
    }
}
