//! The TYT HID protocol, used by the Radioddity GD-77 and its relatives.
//!
//! These radios enumerate as an HID device and the bootloader speaks its own
//! protocol over the endpoints. Every exchange is a command out and a reply
//! in, framed as two little endian sixteen bit fields followed by the
//! payload:
//!
//! ```text
//! 0x00  2  type: 1 from the host, 3 from the radio
//! 0x02  2  payload length
//! 0x04  n  payload
//! ```
//!
//! Two things about the endpoints are worth stating because they are not what
//! the descriptors suggest. The OUT endpoint is bulk, not interrupt, and
//! sending an interrupt transfer to it is rejected outright. The IN endpoint
//! is interrupt and always sends 64 bytes, so asking for fewer risks an
//! overflow. Both of those are recorded in the C++ this follows.

use crate::{Error, Result};

/// Where a command comes from
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Sent by us
    HostToDevice,
    /// Sent by the radio
    DeviceToHost,
}

impl Direction {
    fn as_u16(self) -> u16 {
        match self {
            Self::HostToDevice => 0x01,
            Self::DeviceToHost => 0x03,
        }
    }

    fn from_u16(value: u16) -> Option<Self> {
        match value {
            0x01 => Some(Self::HostToDevice),
            0x03 => Some(Self::DeviceToHost),
            _ => None,
        }
    }
}

/// One framed message
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    /// Which way it is going
    pub direction: Direction,
    /// The payload
    pub data: Vec<u8>,
}

impl Command {
    /// A command from the host
    pub fn host(data: impl Into<Vec<u8>>) -> Self {
        Self {
            direction: Direction::HostToDevice,
            data: data.into(),
        }
    }

    /// Frame it for the wire
    pub fn encode(&self) -> Result<Vec<u8>> {
        let length = u16::try_from(self.data.len())
            .map_err(|_| Error::Port("command payload is too large".to_owned()))?;

        let mut out = Vec::with_capacity(self.data.len() + 4);
        out.extend_from_slice(&self.direction.as_u16().to_le_bytes());
        out.extend_from_slice(&length.to_le_bytes());
        out.extend_from_slice(&self.data);
        Ok(out)
    }

    /// Read a reply.
    ///
    /// The length comes from the radio, so it is checked against what was
    /// actually received rather than trusted to bound a copy.
    pub fn decode(packet: &[u8]) -> Result<Self> {
        let head = packet.get(..4).ok_or(Error::Timeout {
            what: "reply from the radio",
            wanted: 4,
            got: packet.len(),
        })?;

        let raw_type = u16::from_le_bytes([
            *head.first().ok_or(Error::NoRadio)?,
            *head.get(1).ok_or(Error::NoRadio)?,
        ]);
        let length = usize::from(u16::from_le_bytes([
            *head.get(2).ok_or(Error::NoRadio)?,
            *head.get(3).ok_or(Error::NoRadio)?,
        ]));

        let direction = Direction::from_u16(raw_type).ok_or(Error::Unexpected {
            what: "reply from the radio",
            expected: "a command type of 1 or 3".to_owned(),
            got: format!("{raw_type:#06x}"),
        })?;

        let data = packet.get(4..4 + length).ok_or(Error::Unexpected {
            what: "reply from the radio",
            expected: format!("{length} bytes of payload"),
            got: format!("{} bytes", packet.len().saturating_sub(4)),
        })?;

        Ok(Self {
            direction,
            data: data.to_vec(),
        })
    }
}

/// Commands the bootloader understands
pub mod commands {
    /// Acknowledgement, in both directions
    pub const A: &[u8] = b"A";
    /// What the radio answers a download request with
    pub const UPDATE: &[u8] = b"#UPDATE?";
    /// Ask to start
    pub const DOWNLOAD: &[u8] = b"DOWNLOAD";
    /// Program the flash
    pub const FLASH_PROGRAM: &[u8] = b"F-PROG";
    /// Erase the flash
    pub const FLASH_ERASE: &[u8] = b"F-ERASE";
    /// Begin sending firmware
    pub const PROGRAM: &[u8] = b"PROGRAM";
    /// Ends a block, followed by its checksum
    pub const END: &[u8] = b"END";
}

/// A pair of endpoints to talk to a radio over
pub trait HidTransport {
    /// Write a framed command to the bulk out endpoint
    fn write(&mut self, data: &[u8]) -> Result<()>;

    /// Read one 64 byte packet from the interrupt in endpoint
    fn read_packet(&mut self) -> Result<Vec<u8>>;
}

/// A conversation with a radio in bootloader mode
#[derive(Debug)]
pub struct Session<T: HidTransport> {
    transport: T,
}

impl<T: HidTransport> Session<T> {
    /// Start talking to a radio
    pub fn new(transport: T) -> Self {
        Self { transport }
    }

    /// Give the transport back
    pub fn into_inner(self) -> T {
        self.transport
    }

    /// Send a command and read what comes back
    pub fn send(&mut self, command: &Command) -> Result<Command> {
        self.transport.write(&command.encode()?)?;
        let packet = self.transport.read_packet()?;
        Command::decode(&packet)
    }

    /// Send a payload, padded out to `size` with `fill`, as the bootloader
    /// expects for its fixed width commands
    pub fn send_padded(&mut self, data: &[u8], size: usize, fill: u8) -> Result<Command> {
        let mut padded = vec![fill; size.max(data.len())];
        padded
            .get_mut(..data.len())
            .ok_or(Error::Port("padding shorter than the payload".to_owned()))?
            .copy_from_slice(data);
        self.send(&Command::host(padded))
    }

    /// Send something that must be acknowledged
    pub fn send_expecting_ok(&mut self, data: &[u8]) -> Result<()> {
        let reply = self.send(&Command::host(data.to_vec()))?;
        Self::check_ok(&reply)
    }

    /// Send a padded command that must be acknowledged
    pub fn send_padded_expecting_ok(&mut self, data: &[u8], size: usize, fill: u8) -> Result<()> {
        let reply = self.send_padded(data, size, fill)?;
        Self::check_ok(&reply)
    }

    fn check_ok(reply: &Command) -> Result<()> {
        if reply.direction == Direction::DeviceToHost && reply.data == commands::A {
            return Ok(());
        }
        Err(Error::Unexpected {
            what: "acknowledgement",
            expected: "A from the radio".to_owned(),
            got: describe(&reply.data),
        })
    }
}

/// Show a payload as text when it is text, and as hex when it is not
fn describe(data: &[u8]) -> String {
    if data.iter().all(|b| b.is_ascii_graphic() || *b == b' ') && !data.is_empty() {
        format!("{:?}", String::from_utf8_lossy(data))
    } else {
        data.iter()
            .map(|b| format!("{b:02x}"))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// Writing firmware to an SGL radio
pub mod flashing {
    use super::{Command, Direction, HidTransport, Session, commands, describe};
    use crate::{Error, Result};

    /// Bytes of firmware per packet
    const TRANSFER_SIZE: usize = 0x20;
    /// The address and length that precede them
    const HEADER_SIZE: usize = 0x06;
    /// How much is written before a checksum is sent
    const CHECKSUM_BLOCK: usize = 0x400;

    /// What the radio needs to be told about itself before it will take
    /// firmware, all of which comes out of the file's header
    #[derive(Debug, Clone)]
    pub struct Identity {
        /// The key for this model
        pub model_key: Vec<u8>,
        /// Which group of radios it belongs to
        pub radio_group: Vec<u8>,
        /// The model
        pub radio_model: Vec<u8>,
        /// Protocol version
        pub protocol_version: Vec<u8>,
    }

    /// Progress through a flash
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Progress {
        /// Bytes written so far
        pub written: usize,
        /// Bytes in total
        pub total: usize,
    }

    /// The running sum the radio checks each block against
    fn checksum(data: &[u8]) -> u32 {
        data.iter()
            .fold(0u32, |sum, b| sum.wrapping_add(u32::from(*b)))
    }

    /// Write firmware to a radio in bootloader mode.
    ///
    /// `data` is the firmware as stored in the file. It is not deciphered on
    /// the way to the radio, which is what the C++ does and what a TYT radio
    /// over DFU was proved to need.
    pub fn write<T: HidTransport>(
        session: &mut Session<T>,
        identity: &Identity,
        data: &[u8],
        mut progress: impl FnMut(Progress),
    ) -> Result<()> {
        // the radio answers a download request by asking to be updated
        let reply = session.send(&Command::host(commands::DOWNLOAD.to_vec()))?;
        if reply.data != commands::UPDATE {
            return Err(Error::Unexpected {
                what: "the radio's answer to the download command",
                expected: format!("{:?}", String::from_utf8_lossy(commands::UPDATE)),
                got: format!(
                    "{}. Is it in firmware update mode? Power it on holding SK1 and SK2, \
                     the screen stays blank",
                    describe(&reply.data)
                ),
            });
        }
        session.send_expecting_ok(commands::A)?;

        // The radio answers with a prefix of the key rather than the whole
        // thing: send `DV01` followed by the four byte transfer key and it
        // replies `DV01`. So the reply has to be a prefix of what was sent,
        // not the other way round. OpenGD77's loader expects exactly those
        // four bytes, and the C++ compares only as many bytes as the radio
        // returned.
        let key = session.send_padded(&identity.model_key, 0x08, 0xff)?;
        if key.data.is_empty() || !identity.model_key.starts_with(&key.data) {
            return Err(Error::Unexpected {
                what: "the model key",
                expected: describe(&identity.model_key),
                got: format!(
                    "{}, so this firmware is for another radio",
                    describe(&key.data)
                ),
            });
        }

        session.send_padded_expecting_ok(commands::FLASH_PROGRAM, 0x08, 0xff)?;
        session.send_padded_expecting_ok(&identity.radio_group, 0x10, 0xff)?;
        session.send_padded_expecting_ok(&identity.radio_model, 0x08, 0xff)?;
        session.send_expecting_ok(&identity.protocol_version)?;

        session.send_padded_expecting_ok(commands::FLASH_ERASE, 0x08, 0xff)?;
        session.send_expecting_ok(commands::A)?;
        session.send_padded_expecting_ok(commands::PROGRAM, 0x08, 0xff)?;

        let mut address = 0usize;
        let mut block = 0usize;

        while address < data.len() {
            let size = TRANSFER_SIZE.min(data.len() - address);

            // the address and length are big endian here, unlike the framing
            let mut packet = Vec::with_capacity(HEADER_SIZE + size);
            packet.extend_from_slice(&(address as u32).to_be_bytes());
            packet.extend_from_slice(&(size as u16).to_be_bytes());
            packet.extend_from_slice(
                data.get(address..address + size)
                    .ok_or(Error::Port("firmware ran short".to_owned()))?,
            );

            session.send_expecting_ok(&packet)?;
            address += size;

            progress(Progress {
                written: address,
                total: data.len(),
            });

            // every kilobyte, and again at the end, the radio wants the sum
            // of what it has just been given
            if address % CHECKSUM_BLOCK == 0 || address == data.len() {
                let start = CHECKSUM_BLOCK * block;
                let sum = checksum(data.get(start..address).ok_or(Error::Port(
                    "checksum range outside the firmware".to_owned(),
                ))?);

                let mut command = vec![0xffu8; commands::END.len() + 5];
                command
                    .get_mut(..commands::END.len())
                    .ok_or(Error::Port("checksum command too short".to_owned()))?
                    .copy_from_slice(commands::END);
                command
                    .get_mut(commands::END.len() + 1..)
                    .ok_or(Error::Port("checksum command too short".to_owned()))?
                    .copy_from_slice(&sum.to_le_bytes());

                session.send_expecting_ok(&command)?;
                block += 1;
            }
        }

        Ok(())
    }

    /// The acknowledgement a radio sends
    pub fn ok_reply() -> Command {
        Command {
            direction: Direction::DeviceToHost,
            data: commands::A.to_vec(),
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]
mod tests {
    use super::flashing::{Identity, Progress};
    use super::*;

    /// A radio in bootloader mode, which reassembles what it is sent
    struct Radio {
        pub sent: Vec<Command>,
        pub firmware: Vec<u8>,
        pub addresses: Vec<u32>,
        pub checksums: Vec<u32>,
        pub model_key: Vec<u8>,
        replies: Vec<Vec<u8>>,
        stage: usize,
    }

    impl Radio {
        fn new(model_key: &[u8]) -> Self {
            Self {
                sent: Vec::new(),
                firmware: Vec::new(),
                addresses: Vec::new(),
                checksums: Vec::new(),
                model_key: model_key.to_vec(),
                replies: Vec::new(),
                stage: 0,
            }
        }
    }

    impl HidTransport for Radio {
        fn write(&mut self, data: &[u8]) -> Result<()> {
            let command = Command::decode(data)?;
            let payload = command.data.clone();
            self.sent.push(command);

            let reply: Vec<u8> = if payload == commands::DOWNLOAD {
                commands::UPDATE.to_vec()
            } else if payload.starts_with(&self.model_key) && self.stage == 0 {
                self.stage = 1;
                // a real GD-77 answers with the four byte prefix, not the
                // whole key, which OpenGD77's loader documents
                payload[..4].to_vec()
            } else if payload.len() > 6 && payload.starts_with(commands::END) {
                let sum = u32::from_le_bytes([payload[4], payload[5], payload[6], payload[7]]);
                self.checksums.push(sum);
                commands::A.to_vec()
            } else if payload.len() > 6 && !payload.starts_with(b"F-") && self.stage == 1 {
                // a firmware packet: address, length, then the bytes
                let address = u32::from_be_bytes([payload[0], payload[1], payload[2], payload[3]]);
                let length = u16::from_be_bytes([payload[4], payload[5]]) as usize;
                if payload.len() == 6 + length {
                    self.addresses.push(address);
                    self.firmware.extend_from_slice(&payload[6..]);
                }
                commands::A.to_vec()
            } else {
                commands::A.to_vec()
            };

            let framed = Command {
                direction: Direction::DeviceToHost,
                data: reply,
            }
            .encode()?;

            // the radio always sends a full 64 byte packet
            let mut packet = framed;
            packet.resize(64, 0x00);
            self.replies.push(packet);
            Ok(())
        }

        fn read_packet(&mut self) -> Result<Vec<u8>> {
            self.replies
                .pop()
                .ok_or(Error::Port("nothing to read".to_owned()))
        }
    }

    fn identity() -> Identity {
        Identity {
            // as an SGL header holds it: a four byte prefix and the transfer
            // key the radio is sent
            model_key: b"DV01enhi".to_vec(),
            radio_group: b"SK".to_vec(),
            radio_model: b"GD-77".to_vec(),
            protocol_version: b"V01.00".to_vec(),
        }
    }

    #[test]
    fn a_command_frames_and_unframes() {
        let command = Command::host(b"DOWNLOAD".to_vec());
        let wire = command.encode().expect("encodes");

        assert_eq!(
            &wire[..4],
            &[0x01, 0x00, 0x08, 0x00],
            "type then length, little endian"
        );
        assert_eq!(&wire[4..], b"DOWNLOAD");
        assert_eq!(Command::decode(&wire).expect("decodes"), command);
    }

    /// The length in a reply comes from the radio, so it must never be
    /// trusted to bound a copy
    #[test]
    fn a_reply_claiming_more_than_it_sent_is_refused() {
        // says 200 bytes of payload, sends four
        let packet = [0x03, 0x00, 0xc8, 0x00, 1, 2, 3, 4];
        assert!(Command::decode(&packet).is_err());

        assert!(Command::decode(&[]).is_err());
        assert!(Command::decode(&[0x03, 0x00]).is_err());
        // an unknown direction is not guessed at
        assert!(Command::decode(&[0x07, 0x00, 0x00, 0x00]).is_err());
    }

    #[test]
    fn a_reply_padded_out_to_the_packet_size_reads_correctly() {
        // the radio always sends 64 bytes whatever the payload
        let mut packet = Command {
            direction: Direction::DeviceToHost,
            data: b"A".to_vec(),
        }
        .encode()
        .expect("encodes");
        packet.resize(64, 0);

        let command = Command::decode(&packet).expect("decodes");
        assert_eq!(command.data, b"A", "the padding is not part of the payload");
    }

    #[test]
    fn firmware_arrives_in_order_and_intact() {
        let data: Vec<u8> = (0..0x900u32).map(|i| (i % 251) as u8).collect();
        let mut session = Session::new(Radio::new(b"DV01"));

        flashing::write(&mut session, &identity(), &data, |_| {}).expect("writes");

        let radio = session.into_inner();
        assert_eq!(radio.firmware, data, "the firmware did not survive");

        let expected: Vec<u32> = (0..data.len()).step_by(0x20).map(|a| a as u32).collect();
        assert_eq!(radio.addresses, expected, "packets are addressed in order");
    }

    #[test]
    fn a_checksum_follows_every_kilobyte_and_the_end() {
        let data = vec![0x01u8; 0x900];
        let mut session = Session::new(Radio::new(b"DV01"));
        flashing::write(&mut session, &identity(), &data, |_| {}).expect("writes");

        let radio = session.into_inner();
        // two full kilobytes, then the remainder
        assert_eq!(radio.checksums.len(), 3);
        assert_eq!(
            radio.checksums[0], 0x400,
            "a kilobyte of 0x01 sums to 0x400"
        );
        assert_eq!(radio.checksums[1], 0x400);
        assert_eq!(radio.checksums[2], 0x100, "and the last part is shorter");
    }

    #[test]
    fn the_radio_is_asked_to_update_before_anything_else() {
        let data = vec![0u8; 0x20];
        let mut session = Session::new(Radio::new(b"DV01"));
        flashing::write(&mut session, &identity(), &data, |_| {}).expect("writes");

        let radio = session.into_inner();
        assert_eq!(radio.sent[0].data, commands::DOWNLOAD);
        assert_eq!(radio.sent[1].data, commands::A);
        assert!(
            radio.sent[2].data.starts_with(b"DV01"),
            "the model key comes next"
        );
    }

    /// Firmware for another radio must be refused before the flash is erased
    #[test]
    fn a_key_the_radio_does_not_echo_stops_the_flash() {
        let data = vec![0u8; 0x20];
        // the radio answers DV01, the firmware claims a different model
        let mut session = Session::new(Radio::new(b"DV01"));
        let mut wrong = identity();
        wrong.model_key = b"XY99abcd".to_vec();

        let err = flashing::write(&mut session, &wrong, &data, |_| {})
            .expect_err("must not flash the wrong firmware");
        assert!(err.to_string().contains("another radio"), "{err}");

        let radio = session.into_inner();
        assert!(
            !radio
                .sent
                .iter()
                .any(|c| c.data.starts_with(commands::FLASH_ERASE)),
            "nothing may be erased before the key is agreed"
        );
    }

    #[test]
    fn a_radio_that_is_not_in_update_mode_says_so_usefully() {
        struct Confused;
        impl HidTransport for Confused {
            fn write(&mut self, _data: &[u8]) -> Result<()> {
                Ok(())
            }
            fn read_packet(&mut self) -> Result<Vec<u8>> {
                let mut packet = Command {
                    direction: Direction::DeviceToHost,
                    data: b"?".to_vec(),
                }
                .encode()?;
                packet.resize(64, 0);
                Ok(packet)
            }
        }

        let mut session = Session::new(Confused);
        let err = flashing::write(&mut session, &identity(), &[0u8; 0x20], |_| {})
            .expect_err("does not proceed");
        assert!(err.to_string().contains("SK1 and SK2"), "{err}");
    }

    #[test]
    fn progress_adds_up() {
        let data = vec![0u8; 0x840];
        let mut seen = Vec::new();
        let mut session = Session::new(Radio::new(b"DV01"));

        flashing::write(&mut session, &identity(), &data, |p| seen.push(p)).expect("writes");

        assert_eq!(
            seen.last(),
            Some(&Progress {
                written: data.len(),
                total: data.len()
            })
        );
        assert!(seen.windows(2).all(|w| w[0].written < w[1].written));
    }

    /// Vectors taken from OpenGD77's own loader, which talks to the same
    /// radios and is an independent implementation of this protocol
    #[test]
    fn the_framing_matches_opengd77() {
        // its sendAndCheckResponse builds [1, 0, len_lo, len_hi] then the
        // command, and sends DOWNLOAD as eight bytes
        let wire = Command::host(commands::DOWNLOAD.to_vec())
            .encode()
            .expect("encodes");
        assert_eq!(wire[0], 1, "type is one from the host");
        assert_eq!(wire[1], 0);
        assert_eq!(wire[2], 8, "length low byte");
        assert_eq!(wire[3], 0, "length high byte");
        assert_eq!(&wire[4..], b"DOWNLOAD");
    }

    /// Its createChecksumData writes [0x45,0x4e,0x44,0xff] then the sum with
    /// the least significant byte first
    #[test]
    fn the_checksum_frame_matches_opengd77() {
        let data = vec![0x01u8; 0x400];
        let mut session = Session::new(Radio::new(b"DV01"));
        flashing::write(&mut session, &identity(), &data, |_| {}).expect("writes");

        let radio = session.into_inner();
        let end = radio
            .sent
            .iter()
            .find(|c| c.data.starts_with(commands::END))
            .expect("a checksum was sent");

        assert_eq!(end.data.len(), 8);
        assert_eq!(&end.data[..4], &[0x45, 0x4e, 0x44, 0xff], "END then 0xff");

        // 0x400 bytes of 0x01 sum to 0x400, least significant byte first
        assert_eq!(&end.data[4..], &[0x00, 0x04, 0x00, 0x00]);
    }

    /// Its updateBlockAddressAndLength writes the address most significant
    /// byte first, then the length the same way
    #[test]
    fn the_block_header_matches_opengd77() {
        let data = vec![0u8; 0x40];
        let mut session = Session::new(Radio::new(b"DV01"));
        flashing::write(&mut session, &identity(), &data, |_| {}).expect("writes");

        let radio = session.into_inner();
        let blocks: Vec<&Command> = radio.sent.iter().filter(|c| c.data.len() == 0x26).collect();

        assert_eq!(blocks.len(), 2, "0x40 bytes in two packets of 0x20");
        assert_eq!(&blocks[0].data[..6], &[0, 0, 0, 0, 0, 0x20]);
        assert_eq!(
            &blocks[1].data[..6],
            &[0, 0, 0, 0x20, 0, 0x20],
            "address is big endian, so the second packet is at 0x20"
        );
    }

    #[test]
    fn padding_fills_to_the_width_the_bootloader_expects() {
        let mut session = Session::new(Radio::new(b"DV01"));
        session.send_padded(b"F-PROG", 0x08, 0xff).expect("sends");

        let radio = session.into_inner();
        assert_eq!(radio.sent[0].data, b"F-PROG\xff\xff");
    }
}
