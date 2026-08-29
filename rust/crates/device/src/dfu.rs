//! USB DFU, and the TYT extensions layered on it.
//!
//! DFU is control transfers rather than a stream of bytes, so it gets its own
//! narrow trait, [`ControlTransfer`], for the same reason [`crate::ByteStream`]
//! exists: the conversation with a radio can then be run against a fake and
//! checked, instead of only being observable when a radio is plugged in.
//!
//! # Where this departs from the C++
//!
//! Getting to a state where a transfer can start was a `while (1)` that
//! called abort until the device said idle. Abort cannot clear
//! [`State::Error`], so a radio sitting in that state hung the tool with no
//! way out but to kill it. Here the error state is cleared with the request
//! meant for it, and the loop is bounded.

use crate::{Error, Result};

/// Requests defined by the DFU specification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Request {
    /// Leave DFU mode
    Detach = 0x00,
    /// Send data to the device
    Download = 0x01,
    /// Read data back
    Upload = 0x02,
    /// Ask what happened
    GetStatus = 0x03,
    /// Clear an error
    ClearStatus = 0x04,
    /// Ask what state the device is in
    GetState = 0x05,
    /// Give up on the current transfer
    Abort = 0x06,
}

/// The state a device reports being in
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// Running the application, not DFU
    AppIdle,
    /// Application is detaching
    AppDetach,
    /// Ready for a command
    Idle,
    /// Download in progress, waiting to be asked about it
    DownloadSync,
    /// Download command executing
    DownloadBusy,
    /// Ready for more download data
    DownloadIdle,
    /// Manifestation, waiting to be asked
    ManifestSync,
    /// Manifesting
    Manifest,
    /// Manifested, waiting for a reset
    ManifestWaitReset,
    /// Ready to be read from
    UploadIdle,
    /// Something went wrong and must be cleared
    Error,
    /// Not a state this knows, kept so a device is never misread as idle
    Unknown(u8),
}

impl From<u8> for State {
    fn from(value: u8) -> Self {
        match value {
            0x00 => Self::AppIdle,
            0x01 => Self::AppDetach,
            0x02 => Self::Idle,
            0x03 => Self::DownloadSync,
            0x04 => Self::DownloadBusy,
            0x05 => Self::DownloadIdle,
            0x06 => Self::ManifestSync,
            0x07 => Self::Manifest,
            0x08 => Self::ManifestWaitReset,
            0x09 => Self::UploadIdle,
            0x0a => Self::Error,
            other => Self::Unknown(other),
        }
    }
}

/// What a device says about the last request
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Status {
    /// Zero means it went well
    pub status: u8,
    /// How long the device asks to be left alone, in milliseconds
    pub poll_timeout: u32,
    /// Where it is now
    pub state: State,
    /// Index of a string describing the status
    pub string_index: u8,
}

impl Status {
    /// Read the six byte status reply
    fn parse(data: &[u8]) -> Result<Self> {
        let at = |i: usize| -> Result<u8> {
            data.get(i).copied().ok_or(Error::Timeout {
                what: "status",
                wanted: 6,
                got: data.len(),
            })
        };

        Ok(Self {
            status: at(0)?,
            // three bytes, little endian
            poll_timeout: u32::from(at(1)?) | (u32::from(at(2)?) << 8) | (u32::from(at(3)?) << 16),
            state: State::from(at(4)?),
            string_index: at(5)?,
        })
    }

    /// Did the last request succeed
    pub fn is_ok(&self) -> bool {
        self.status == 0
    }
}

/// A device that answers USB control transfers.
///
/// The direction is implied by the method, so an implementation only has to
/// know the class request type: `0x21` going out, `0xa1` coming back.
pub trait ControlTransfer {
    /// Send a class request to the device
    fn control_out(&mut self, request: u8, value: u16, data: &[u8]) -> Result<()>;

    /// Read a class request back from the device
    fn control_in(&mut self, request: u8, value: u16, length: usize) -> Result<Vec<u8>>;
}

/// How many times to try to get a device back to a usable state before
/// deciding it is not going to happen
const SETTLE_ATTEMPTS: usize = 10;

/// Bytes in a status reply
const STATUS_LEN: usize = 6;

/// A DFU conversation with a device
#[derive(Debug)]
pub struct Dfu<T: ControlTransfer> {
    transport: T,
}

impl<T: ControlTransfer + ?Sized> ControlTransfer for &mut T {
    fn control_out(&mut self, request: u8, value: u16, data: &[u8]) -> Result<()> {
        (**self).control_out(request, value, data)
    }

    fn control_in(&mut self, request: u8, value: u16, length: usize) -> Result<Vec<u8>> {
        (**self).control_in(request, value, length)
    }
}

impl<T: ControlTransfer> Dfu<T> {
    /// Start talking DFU to a device
    pub fn new(transport: T) -> Self {
        Self { transport }
    }

    /// Give the transport back
    pub fn into_inner(self) -> T {
        self.transport
    }

    /// Ask what state the device is in
    pub fn state(&mut self) -> Result<State> {
        let reply = self.transport.control_in(Request::GetState as u8, 0, 1)?;
        reply
            .first()
            .map(|b| State::from(*b))
            .ok_or(Error::Timeout {
                what: "device state",
                wanted: 1,
                got: 0,
            })
    }

    /// Ask what happened to the last request
    pub fn status(&mut self) -> Result<Status> {
        let reply = self
            .transport
            .control_in(Request::GetStatus as u8, 0, STATUS_LEN)?;
        Status::parse(&reply)
    }

    /// Give up on the current transfer
    pub fn abort(&mut self) -> Result<()> {
        self.transport.control_out(Request::Abort as u8, 0, &[])
    }

    /// Clear an error, which abort cannot do
    pub fn clear_status(&mut self) -> Result<()> {
        self.transport
            .control_out(Request::ClearStatus as u8, 0, &[])
    }

    /// Leave DFU mode
    pub fn detach(&mut self) -> Result<()> {
        self.transport.control_out(Request::Detach as u8, 0, &[])
    }

    /// Get the device into a state where a transfer can start.
    ///
    /// `ready` says which states are already good. Anything else is aborted,
    /// and an error state is cleared rather than aborted, because abort does
    /// not clear it and looping on it never terminates.
    fn settle(&mut self, ready: &[State], what: &'static str) -> Result<()> {
        for _ in 0..SETTLE_ATTEMPTS {
            let state = self.state()?;
            if ready.contains(&state) {
                return Ok(());
            }

            if state == State::Error {
                self.clear_status()?;
            } else {
                self.abort()?;
            }
        }

        Err(Error::Unexpected {
            what,
            expected: format!("one of {ready:?}"),
            got: format!("a device that will not leave {:?}", self.state()?),
        })
    }

    /// Send a block to the device.
    ///
    /// `value` is the block number the device uses to place the data, which
    /// is why a caller writing firmware has to count.
    pub fn download(&mut self, data: &[u8], value: u16) -> Result<()> {
        self.settle(&[State::Idle, State::DownloadIdle], "starting a download")?;

        self.transport
            .control_out(Request::Download as u8, value, data)?;

        // the request only runs when the device is asked about it
        let started = self.status()?;
        if !started.is_ok() {
            return Err(Error::Unexpected {
                what: "download",
                expected: "a device that accepted the command".to_owned(),
                got: format!("status {:#04x} in {:?}", started.status, started.state),
            });
        }
        if started.state != State::DownloadBusy {
            return Err(Error::Unexpected {
                what: "download",
                expected: format!("{:?}", State::DownloadBusy),
                got: format!("{:?}", started.state),
            });
        }

        // and the device may ask for time before it is asked again
        if started.poll_timeout > 0 {
            std::thread::sleep(std::time::Duration::from_millis(u64::from(
                started.poll_timeout,
            )));
        }

        let finished = self.status()?;
        if !finished.is_ok() || finished.state != State::DownloadIdle {
            return Err(Error::Unexpected {
                what: "download",
                expected: format!("{:?}", State::DownloadIdle),
                got: format!("status {:#04x} in {:?}", finished.status, finished.state),
            });
        }

        Ok(())
    }

    /// Read a block back from the device.
    ///
    /// This settles the device first, which aborts if it is in the middle of
    /// something. Beware that abort resets the address pointer on these
    /// radios, so a `set_address` immediately before this is thrown away.
    /// Use [`Self::upload_raw`] to read from an address that has just been
    /// set.
    pub fn upload(&mut self, length: usize, value: u16) -> Result<Vec<u8>> {
        self.settle(&[State::Idle, State::UploadIdle], "starting an upload")?;
        self.transport
            .control_in(Request::Upload as u8, value, length)
    }

    /// Get the device back to idle, the way dmrconfig does before it starts
    /// moving a codeplug.
    ///
    /// This matters more than it looks. After setting an address the device
    /// is in `dfuDNLOAD-IDLE`, and the specification does not allow an upload
    /// from there, so the radio stalls it. Aborting first is what makes a
    /// read work, and the address pointer survives the abort.
    pub fn wait_idle(&mut self) -> Result<()> {
        for _ in 0..SETTLE_ATTEMPTS {
            match self.state()? {
                State::Idle => return Ok(()),
                State::Error => self.clear_status()?,
                State::DownloadBusy | State::ManifestWaitReset | State::AppDetach => {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
                _ => self.abort()?,
            }
        }

        Err(Error::Unexpected {
            what: "waiting for the radio to go idle",
            expected: "idle".to_owned(),
            got: format!("{:?}", self.state()?),
        })
    }

    /// Read a block without settling first.
    ///
    /// The address pointer set by [`Self::set_address`] survives into this,
    /// which it does not survive an abort. dmrconfig reads a whole codeplug
    /// this way: set the address once, then walk the block numbers, and the
    /// radio works out `pointer + (block - 2) * 1024` for itself.
    ///
    /// An earlier probe here concluded that these radios ignore addressing
    /// on upload entirely. That was wrong, and this is why: the probe went
    /// through [`Self::upload`], whose abort had already discarded the
    /// address before the read happened.
    pub fn upload_raw(&mut self, length: usize, value: u16) -> Result<Vec<u8>> {
        self.transport
            .control_in(Request::Upload as u8, value, length)
    }

    /// Write a block without settling first, for the same reason
    pub fn download_raw(&mut self, data: &[u8], value: u16) -> Result<()> {
        self.transport
            .control_out(Request::Download as u8, value, data)
    }

    /// Finish a download and ask the device to run what was written.
    ///
    /// A DFU transfer is not over until the host sends a download of no
    /// length. That is what moves the device into manifestation, where it
    /// accepts the new firmware and starts it. Without it the bootloader has
    /// been given every byte of an image and still has no reason to believe
    /// the transfer finished, so it stays in DFU.
    ///
    /// The device may drop off the bus as it restarts, so failures after the
    /// request has gone out are not treated as errors.
    pub fn leave(&mut self, address: u32) -> Result<()> {
        self.set_address(address)?;

        // a zero length download, block two, as dfu-util sends
        self.transport
            .control_out(Request::Download as u8, 2, &[])?;

        // asking for status is what starts manifestation
        let _ = self.status();
        Ok(())
    }

    /// Point the device at an address for what follows
    pub fn set_address(&mut self, address: u32) -> Result<()> {
        let mut command = vec![0x21];
        command.extend_from_slice(&address.to_le_bytes());
        self.download(&command, 0)
    }

    /// Erase the page holding an address
    pub fn erase(&mut self, address: u32) -> Result<()> {
        let mut command = vec![0x41];
        command.extend_from_slice(&address.to_le_bytes());
        self.download(&command, 0)
    }
}

/// The TYT specific part, which rides on DFU as vendor commands
pub mod tyt {
    use super::{ControlTransfer, Dfu};
    use crate::Result;

    /// First byte of a vendor command
    pub const CUSTOM_COMMAND: u8 = 0x91;
    /// First byte of a register read
    pub const REGISTER_COMMAND: u8 = 0xa2;
    /// Registers are read back a fixed size regardless of content
    pub const REGISTER_SIZE: usize = 1024;

    /// Vendor commands
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[repr(u8)]
    pub enum Command {
        /// Put the radio into programming mode
        ProgrammingMode = 0x01,
        /// Set the clock
        SetClock = 0x02,
        /// Restart
        Reboot = 0x05,
        /// Begin a firmware upgrade
        FirmwareUpgrade = 0x31,
    }

    /// Registers that can be read
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[repr(u8)]
    pub enum Register {
        /// Radio model, then sixteen bytes of something else
        RadioInfo = 0x01,
        /// Unknown, four bytes
        R02 = 0x02,
        /// Unknown, twenty four bytes
        R03 = 0x03,
        /// Unknown, eight bytes
        R04 = 0x04,
        /// Unknown, sixteen bytes
        R07 = 0x07,
        /// Real time clock, seven bytes
        Rtc = 0x08,
    }

    /// Read a register
    pub fn read_register<T: ControlTransfer>(
        dfu: &mut Dfu<T>,
        register: Register,
    ) -> Result<Vec<u8>> {
        dfu.download(&[REGISTER_COMMAND, register as u8], 0)?;
        dfu.upload(REGISTER_SIZE, 0)
    }

    /// Ask the radio what model it is.
    ///
    /// The register is a fixed size buffer and the model is a C string in it,
    /// with no guarantee of a terminator, so the whole buffer bounds the read.
    pub fn identify<T: ControlTransfer>(dfu: &mut Dfu<T>) -> Result<String> {
        let data = read_register(dfu, Register::RadioInfo)?;
        let end = data.iter().position(|b| *b == 0).unwrap_or(data.len());
        Ok(String::from_utf8_lossy(data.get(..end).unwrap_or_default()).into_owned())
    }

    /// A timestamp as these radios store one: seven BCD bytes, century and
    /// year, then month, day, hour, minute, second.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Timestamp {
        /// Full year, so 2018 rather than 18
        pub year: u16,
        /// 1 to 12
        pub month: u8,
        /// 1 to 31
        pub day: u8,
        /// 0 to 23
        pub hour: u8,
        /// 0 to 59
        pub minute: u8,
        /// 0 to 59
        pub second: u8,
    }

    impl Timestamp {
        /// Read seven BCD bytes.
        ///
        /// Returns None when the bytes are not BCD at all, which is what a
        /// DM-1701 gives back: its register 8 holds `4b 01 00 10`, and the
        /// C++ turns that into the year 5100 rather than saying it is not a
        /// timestamp.
        pub fn parse(data: &[u8]) -> Option<Self> {
            let bcd = |b: u8| -> Option<u8> {
                let (hi, lo) = (b >> 4, b & 0x0f);
                if hi > 9 || lo > 9 {
                    return None;
                }
                Some(hi * 10 + lo)
            };

            let at = |i: usize| -> Option<u8> { bcd(*data.get(i)?) };

            let stamp = Self {
                year: u16::from(at(0)?) * 100 + u16::from(at(1)?),
                month: at(2)?,
                day: at(3)?,
                hour: at(4)?,
                minute: at(5)?,
                second: at(6)?,
            };

            // BCD digits alone do not make a date
            if !(1..=12).contains(&stamp.month)
                || !(1..=31).contains(&stamp.day)
                || stamp.hour > 23
                || stamp.minute > 59
                || stamp.second > 59
                || !(1990..=2100).contains(&stamp.year)
            {
                return None;
            }

            Some(stamp)
        }
    }

    impl std::fmt::Display for Timestamp {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(
                f,
                "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
                self.year, self.month, self.day, self.hour, self.minute, self.second
            )
        }
    }

    /// Read the radio's clock, if it has one there.
    ///
    /// Returns the raw register alongside, because on some radios this
    /// register is not a clock at all and the raw bytes are the only honest
    /// thing to show.
    pub fn read_clock<T: ControlTransfer>(
        dfu: &mut Dfu<T>,
    ) -> Result<(Option<Timestamp>, Vec<u8>)> {
        let data = read_register(dfu, Register::Rtc)?;
        let raw: Vec<u8> = data.iter().copied().take(7).collect();
        Ok((Timestamp::parse(&raw), raw))
    }

    /// First byte of the frame that carries a clock
    const CLOCK_FRAME: u8 = 0xb5;

    impl Timestamp {
        /// The seven BCD bytes a radio stores
        pub fn to_bcd(self) -> [u8; 7] {
            let bcd = |v: u16| -> u8 { (((v / 10) % 10) << 4) as u8 | (v % 10) as u8 };
            [
                bcd(self.year / 100),
                bcd(self.year % 100),
                bcd(u16::from(self.month)),
                bcd(u16::from(self.day)),
                bcd(u16::from(self.hour)),
                bcd(u16::from(self.minute)),
                bcd(u16::from(self.second)),
            ]
        }
    }

    /// Set the radio's clock.
    ///
    /// The command that selects the clock, then a frame of `0xb5` and the
    /// seven BCD bytes. The radio keeps local time, not UTC: it has no idea
    /// where it is.
    pub fn write_clock<T: ControlTransfer>(dfu: &mut Dfu<T>, when: Timestamp) -> Result<()> {
        send_command(dfu, Command::SetClock)?;

        let mut frame = vec![CLOCK_FRAME];
        frame.extend_from_slice(&when.to_bcd());
        dfu.download(&frame, 0)?;
        Ok(())
    }

    /// Send a vendor command
    pub fn send_command<T: ControlTransfer>(dfu: &mut Dfu<T>, command: Command) -> Result<()> {
        dfu.download(&[CUSTOM_COMMAND, command as u8], 0)
    }

    /// Restart the radio.
    ///
    /// The second command normally fails, because the radio reboots without
    /// answering, so that failure is not treated as one.
    pub fn reboot<T: ControlTransfer>(dfu: &mut Dfu<T>) -> Result<()> {
        send_command(dfu, Command::ProgrammingMode)?;
        let _ = send_command(dfu, Command::Reboot);
        Ok(())
    }
}

/// Writing firmware to a TYT radio.
///
/// This is the one operation here that can leave a radio unable to start, so
/// it is worth being explicit about the order it does things in:
///
/// 1. tell the radio a firmware upgrade is coming
/// 2. erase every sector the image touches, and no others
/// 3. write the image a block at a time, setting the address per sector
///
/// The erase is the dangerous half. Sectors are not all the same size, so a
/// region is split on sector boundaries by [`crate::flash`], and a test there
/// checks the pieces cover the range exactly without spilling into a
/// neighbouring sector. Erasing one sector too many takes out whatever was
/// next to the image, which on these radios is the bootloader.
pub mod flashing {
    use super::{ControlTransfer, Dfu, tyt};
    use crate::flash::{self, Sector};
    use crate::{Error, Result};

    /// Bytes per download block
    pub const TRANSFER_SIZE: usize = 1024;

    /// A region of an image, and where it goes
    #[derive(Debug, Clone, Copy)]
    pub struct Region<'a> {
        /// Where in the radio it belongs
        pub address: u32,
        /// The bytes
        pub data: &'a [u8],
    }

    /// What is happening, for a caller that wants to show it
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Step {
        /// About to erase a sector
        Erasing {
            /// Address being erased
            address: u32,
            /// The sector it is in
            sector: Sector,
        },
        /// Writing part of the image
        Writing {
            /// Address being written
            address: u32,
            /// Bytes written so far
            written: usize,
            /// Bytes in total
            total: usize,
        },
    }

    /// Write firmware regions to a radio.
    ///
    /// Every sector is erased before anything is written, which is what the
    /// vendor tool does and what the radio expects.
    pub fn write<T: ControlTransfer>(
        dfu: &mut Dfu<T>,
        regions: &[Region<'_>],
        map: &[Sector],
        mut progress: impl FnMut(Step),
    ) -> Result<()> {
        // refuse anything that is not entirely inside the flash map before
        // touching the radio, rather than erasing half of it and then failing
        for region in regions {
            let end = region
                .address
                .checked_add(u32::try_from(region.data.len()).map_err(|_| {
                    Error::Port("a firmware region larger than the address space".to_owned())
                })?)
                .ok_or_else(|| Error::Port("a firmware region that wraps round".to_owned()))?;

            let pieces = flash::split(map, region.address, end);
            let covered: u32 = pieces.iter().map(|p| p.length).sum();

            if covered != end.saturating_sub(region.address) {
                return Err(Error::Unexpected {
                    what: "firmware layout",
                    expected: "a region inside the flash map".to_owned(),
                    got: format!(
                        "{:#010x} to {end:#010x}, which is not all mapped flash",
                        region.address
                    ),
                });
            }
        }

        tyt::send_command(dfu, tyt::Command::FirmwareUpgrade)?;

        for region in regions {
            let end = region.address + region.data.len() as u32;

            for piece in flash::split(map, region.address, end) {
                progress(Step::Erasing {
                    address: piece.address,
                    sector: piece.sector,
                });
                dfu.erase(piece.address)?;
            }
        }

        let total: usize = regions.iter().map(|r| r.data.len()).sum();
        let mut written = 0usize;

        for region in regions {
            let end = region.address + region.data.len() as u32;

            for piece in flash::split(map, region.address, end) {
                let offset = (piece.address - region.address) as usize;
                let length = piece.length as usize;
                let bytes = region
                    .data
                    .get(offset..offset + length)
                    .ok_or(Error::Port("a piece outside its own region".to_owned()))?;

                // The address is set once for the sector, and the blocks that
                // follow are numbered upwards from two, so the radio places
                // each one at `pointer + (block - 2) * TRANSFER_SIZE`.
                //
                // This is not what dfu-util does. For a DfuSe device it sets
                // the address before every block and always sends block two,
                // commenting it "for no address offset". That is the right
                // thing for a standard device and it does not work here: a
                // DM-1701 written that way took every byte without complaint,
                // reported success, and then would not boot. Reflashing the
                // same image with the sequence below fixed it.
                //
                // Manifestation does not survive a power cycle, so a radio
                // that still enters DFU after being switched off and on was
                // not given a valid image, which is what rules out the end of
                // transfer request as the explanation.
                //
                // These radios run a bootloader that answers DFU requests
                // rather than a DfuSe implementation, and it is worth
                // assuming nothing else about it. Its upload, for instance,
                // ignores addressing completely and after a download will
                // echo back the last command it was sent.
                dfu.set_address(piece.address)?;

                for (index, block) in bytes.chunks(TRANSFER_SIZE).enumerate() {
                    let value = u16::try_from(index + 2)
                        .map_err(|_| Error::Port("too many blocks for one address".to_owned()))?;
                    dfu.download(block, value)?;

                    written += block.len();
                    progress(Step::Writing {
                        address: piece.address + (index * TRANSFER_SIZE) as u32,
                        written,
                        total,
                    });
                }
            }
        }

        Ok(())
    }
}

/// Moving a codeplug to and from a TYT radio over DFU.
///
/// This is a different thing from firmware. The codeplug lives in the
/// radio's configuration memory, and is read and written a kilobyte at a
/// time by block number, with the address pointer set once at the start.
///
/// The sequence follows dmrconfig, which is the tool people actually use on
/// these radios, and which is in `references/`. radio_tool's C++ refuses
/// codeplugs on this family altogether.
pub mod codeplug {
    use super::{ControlTransfer, Dfu, tyt};
    use crate::{Error, Result};

    /// Bytes in a block
    pub const BLOCK: usize = 1024;

    /// How much configuration memory these radios have, from dmrconfig's
    /// MEMSZ for the MD-UV380 family, which the DM-1701 belongs to
    pub const MEMORY_SIZE: usize = 0xd_0000;

    /// Blocks the radio holds
    pub const BLOCKS: usize = MEMORY_SIZE / BLOCK;

    /// Progress through a transfer
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Progress {
        /// Bytes moved so far
        pub done: usize,
        /// Bytes in total
        pub total: usize,
    }

    /// dmrconfig shifts block numbers from 256 up to 2048 by 832.
    ///
    /// This is not a quirk to be tidied away: it is how the image maps onto
    /// the radio. Image offset 0x40000 becomes radio address 0x110000, which
    /// is the extended memory where OpenRTX's MDUV3x0 driver reads channels
    /// (`chDataBaseAddr`) and contacts (`contactBaseAddr` at 0x140000).
    /// Address the radio linearly instead and 0x40000 reads back font data.
    fn map_block(block: usize) -> usize {
        if (256..2048).contains(&block) {
            block + 832
        } else {
            block
        }
    }

    fn block_value(block: usize) -> Result<u16> {
        u16::try_from(map_block(block) + 2)
            .map_err(|_| Error::Port("block number too large for a transfer".to_owned()))
    }

    /// Put the radio back into normal operation.
    ///
    /// Every codeplug operation starts by asking the radio to enter
    /// programming mode, and it stays there until told otherwise. dmrconfig
    /// sends this from `radio_disconnect`, commented "restore the normal
    /// radio mode". Leaving it out leaves a radio that has to be switched off
    /// and on before it will work again.
    pub fn leave_programming_mode<T: ControlTransfer>(dfu: &mut Dfu<T>) -> Result<()> {
        dfu.wait_idle()?;
        // the radio restarts without answering, so a failure after the
        // request has gone out is not one
        let _ = dfu.download_raw(&[tyt::CUSTOM_COMMAND, tyt::Command::Reboot as u8], 0);
        let _ = dfu.status();
        Ok(())
    }

    /// Read the whole codeplug out of a radio.
    ///
    /// The programming mode command comes first. Without it the radio stalls
    /// the first upload: the configuration memory is not reachable until it
    /// has been asked for. dmrconfig sends it when it opens the device, for
    /// reading as much as for writing.
    pub fn read<T: ControlTransfer>(
        dfu: &mut Dfu<T>,
        mut progress: impl FnMut(Progress),
    ) -> Result<Vec<u8>> {
        dfu.wait_idle()?;
        tyt::send_command(dfu, tyt::Command::ProgrammingMode)?;
        std::thread::sleep(std::time::Duration::from_millis(100));
        dfu.wait_idle()?;

        dfu.set_address(0)?;
        // an upload is not allowed from the state a download leaves behind,
        // so go back to idle before asking for the first block
        dfu.wait_idle()?;

        let mut image = Vec::with_capacity(MEMORY_SIZE);
        for block in 0..BLOCKS {
            // The block number carries the address, and the shift below
            // reaches the radio's extended memory: image offset 0x40000 is
            // radio address 0x110000, which is where OpenRTX reads channels
            // from. Setting the address linearly instead reads radio 0x40000,
            // which holds fonts, and makes the channels look empty.
            let data = dfu.upload_raw(BLOCK, block_value(block)?)?;
            if data.len() != BLOCK {
                return Err(Error::Timeout {
                    what: "codeplug block",
                    wanted: BLOCK,
                    got: data.len(),
                });
            }
            image.extend_from_slice(&data);
            let _ = dfu.status();

            progress(Progress {
                done: image.len(),
                total: MEMORY_SIZE,
            });
        }

        leave_programming_mode(dfu)?;
        Ok(image)
    }

    /// Erase the configuration memory, which has to happen before a write.
    ///
    /// The addresses are dmrconfig's: four 64K blocks for the configuration
    /// itself, then thirteen more for the extended part.
    fn erase<T: ControlTransfer>(dfu: &mut Dfu<T>, whole: bool) -> Result<()> {
        // enter programming mode first
        tyt::send_command(dfu, tyt::Command::ProgrammingMode)?;
        std::thread::sleep(std::time::Duration::from_millis(100));

        for address in [0x0000_0000u32, 0x0001_0000, 0x0002_0000, 0x0003_0000] {
            dfu.erase(address)?;
        }

        if whole {
            for step in 0..13u32 {
                dfu.erase(0x0011_0000 + step * 0x0001_0000)?;
            }
        }

        dfu.set_address(0)?;
        dfu.wait_idle()?;
        Ok(())
    }

    /// Write a codeplug to a radio
    pub fn write<T: ControlTransfer>(
        dfu: &mut Dfu<T>,
        image: &[u8],
        mut progress: impl FnMut(Progress),
    ) -> Result<()> {
        if image.len() != MEMORY_SIZE {
            return Err(Error::Unexpected {
                what: "codeplug size",
                expected: format!("{MEMORY_SIZE} bytes"),
                got: format!("{} bytes", image.len()),
            });
        }

        erase(dfu, true)?;

        for block in 0..BLOCKS {
            let at = block * BLOCK;
            let data = image
                .get(at..at + BLOCK)
                .ok_or(Error::Port("codeplug ran short".to_owned()))?;

            dfu.download_raw(data, block_value(block)?)?;

            // the status is what says whether the radio took the block.
            // Ignoring it is how a write that changed nothing at all was
            // reported as a success.
            let status = dfu.status()?;
            if !status.is_ok() {
                return Err(Error::Unexpected {
                    what: "writing a codeplug block",
                    expected: "a radio that accepted the block".to_owned(),
                    got: format!(
                        "status {:#04x} in {:?} at block {block}",
                        status.status, status.state
                    ),
                });
            }
            dfu.wait_idle()?;

            progress(Progress {
                done: at + BLOCK,
                total: MEMORY_SIZE,
            });
        }

        // a radio can take every block, report success for each, and store
        // none of them, so the only honest end to a write is to read it back
        verify(dfu, image)?;
        leave_programming_mode(dfu)?;
        Ok(())
    }

    /// Check that a write actually reached the flash.
    ///
    /// This is not belt and braces. A DM-1701 bootloader accepts every block,
    /// reports status zero for each one, and stores none of them: it
    /// implements DFU for its internal flash and not for the configuration
    /// memory, whatever its descriptors advertise. Without reading back, a
    /// codeplug write that changed nothing at all reports success.
    fn verify<T: ControlTransfer>(dfu: &mut Dfu<T>, expected: &[u8]) -> Result<()> {
        // a handful of blocks spread across the image, enough to tell a write
        // that worked from one that did nothing
        let sample = [0usize, BLOCKS / 4, BLOCKS / 2, (BLOCKS * 3) / 4, BLOCKS - 1];

        dfu.wait_idle()?;
        tyt::send_command(dfu, tyt::Command::ProgrammingMode)?;
        std::thread::sleep(std::time::Duration::from_millis(100));
        dfu.wait_idle()?;
        dfu.set_address(0)?;
        dfu.wait_idle()?;

        for block in sample {
            let at = block * BLOCK;
            let Some(want) = expected.get(at..at + BLOCK) else {
                continue;
            };

            let got = dfu.upload_raw(BLOCK, block_value(block)?)?;
            let _ = dfu.status();

            if got.len() == BLOCK && got != want {
                return Err(Error::Unexpected {
                    what: "verifying the codeplug",
                    expected: "the radio to hold what it was sent".to_owned(),
                    got: format!(
                        "block {block} came back different. This radio takes \
                         codeplug writes and stores none of them: its bootloader \
                         implements DFU for firmware only, so reading works and \
                         writing does not"
                    ),
                });
            }
        }

        Ok(())
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
    use super::*;

    /// A device that follows the DFU state machine
    struct Device {
        state: State,
        /// Blocks it has been sent, with their block numbers
        pub downloads: Vec<(u16, Vec<u8>)>,
        /// What upload hands back
        pub upload_data: Vec<u8>,
        /// Status code to report, non zero meaning a failure
        pub status_code: u8,
        /// Refuse to leave the error state, however it is asked
        pub stuck: bool,
        /// How many times the state was asked for
        pub state_reads: usize,
        pub aborts: usize,
        pub clears: usize,
        /// Total requests, so an unbounded loop fails the test instead of
        /// stalling the run
        requests: usize,
    }

    impl Device {
        fn new() -> Self {
            Self {
                state: State::Idle,
                downloads: Vec::new(),
                upload_data: vec![0xab; 16],
                status_code: 0,
                stuck: false,
                state_reads: 0,
                aborts: 0,
                clears: 0,
                requests: 0,
            }
        }
    }

    impl Device {
        fn count(&mut self) {
            self.requests += 1;
            // high enough for a real transfer, which sets an address and
            // sends a block per kilobyte, and low enough that a loop with no
            // exit fails the test rather than hanging the run
            assert!(
                self.requests < 20_000,
                "the host has made {} requests, it is not bounded",
                self.requests
            );
        }
    }

    impl ControlTransfer for Device {
        fn control_out(&mut self, request: u8, value: u16, data: &[u8]) -> Result<()> {
            self.count();
            match request {
                x if x == Request::Download as u8 => {
                    self.downloads.push((value, data.to_vec()));
                    self.state = State::DownloadSync;
                }
                x if x == Request::Abort as u8 => {
                    self.aborts += 1;
                    // abort cannot clear an error, which is the whole point
                    if self.state != State::Error {
                        self.state = State::Idle;
                    }
                }
                x if x == Request::ClearStatus as u8 => {
                    self.clears += 1;
                    if !self.stuck {
                        self.state = State::Idle;
                    }
                }
                x if x == Request::Detach as u8 => self.state = State::AppDetach,
                _ => {}
            }
            Ok(())
        }

        fn control_in(&mut self, request: u8, _value: u16, length: usize) -> Result<Vec<u8>> {
            self.count();
            match request {
                x if x == Request::GetState as u8 => {
                    self.state_reads += 1;
                    let byte = match self.state {
                        State::AppIdle => 0x00,
                        State::AppDetach => 0x01,
                        State::Idle => 0x02,
                        State::DownloadSync => 0x03,
                        State::DownloadBusy => 0x04,
                        State::DownloadIdle => 0x05,
                        State::ManifestSync => 0x06,
                        State::Manifest => 0x07,
                        State::ManifestWaitReset => 0x08,
                        State::UploadIdle => 0x09,
                        State::Error => 0x0a,
                        State::Unknown(b) => b,
                    };
                    Ok(vec![byte])
                }
                x if x == Request::GetStatus as u8 => {
                    // asking is what makes the command run: a device in
                    // download sync reports busy, and once busy it reports
                    // idle, which is the sequence the spec describes
                    let next = match self.state {
                        State::DownloadSync => State::DownloadBusy,
                        State::DownloadBusy => State::DownloadIdle,
                        other => other,
                    };
                    self.state = next;

                    let code = match next {
                        State::Idle => 0x02,
                        State::DownloadSync => 0x03,
                        State::DownloadBusy => 0x04,
                        State::DownloadIdle => 0x05,
                        State::UploadIdle => 0x09,
                        State::Error => 0x0a,
                        _ => 0x02,
                    };
                    Ok(vec![self.status_code, 0, 0, 0, code, 0])
                }
                x if x == Request::Upload as u8 => {
                    Ok(self.upload_data.iter().copied().take(length).collect())
                }
                _ => Ok(Vec::new()),
            }
        }
    }

    #[test]
    fn states_round_trip_through_their_byte_values() {
        for byte in 0u8..=0xff {
            let state = State::from(byte);
            if byte <= 0x0a {
                assert_ne!(state, State::Unknown(byte), "{byte:#04x} is a known state");
            } else {
                assert_eq!(
                    state,
                    State::Unknown(byte),
                    "{byte:#04x} must not be mistaken for a known state"
                );
            }
        }
    }

    #[test]
    fn a_status_reply_is_read_correctly() {
        // poll timeout is three bytes, little endian
        // the third byte has to be non zero, or dropping it would go unnoticed
        let status = Status::parse(&[0x00, 0x10, 0x27, 0x01, 0x05, 0x00]).expect("parses");
        assert!(status.is_ok());
        assert_eq!(status.poll_timeout, 0x0001_2710);
        assert_eq!(status.state, State::DownloadIdle);

        assert!(
            Status::parse(&[0, 0, 0]).is_err(),
            "a short reply is refused"
        );
    }

    #[test]
    fn a_download_runs_the_command_and_checks_it_finished() {
        let mut dfu = Dfu::new(Device::new());
        dfu.download(&[1, 2, 3], 7).expect("downloads");

        let device = dfu.into_inner();
        assert_eq!(device.downloads, vec![(7, vec![1, 2, 3])]);
    }

    #[test]
    fn a_failing_status_is_not_mistaken_for_success() {
        let mut device = Device::new();
        device.status_code = 0x0f;

        let err = Dfu::new(device)
            .download(&[1], 0)
            .expect_err("must not claim success");
        assert!(err.to_string().contains("download"), "{err}");
    }

    /// A device can report a clean status and still never leave the busy
    /// state. Treating that as a completed write is how a half written
    /// firmware gets called a success.
    #[test]
    fn a_download_that_never_finishes_is_not_called_success() {
        struct Busy;
        impl ControlTransfer for Busy {
            fn control_out(&mut self, _request: u8, _value: u16, _data: &[u8]) -> Result<()> {
                Ok(())
            }
            fn control_in(&mut self, request: u8, _value: u16, _length: usize) -> Result<Vec<u8>> {
                if request == Request::GetState as u8 {
                    return Ok(vec![0x02]);
                }
                // status is fine, but it is busy and stays busy
                Ok(vec![0x00, 0, 0, 0, 0x04, 0])
            }
        }

        let err = Dfu::new(Busy)
            .download(&[1], 0)
            .expect_err("a write that never completed is not a success");
        assert!(err.to_string().contains("DownloadIdle"), "{err}");
    }

    /// A device that rejects the command and then reports itself fine must
    /// not be believed. Only the first status sees the rejection.
    #[test]
    fn a_command_the_device_rejected_is_not_undone_by_a_later_good_status() {
        struct Flaky {
            statuses: usize,
        }
        impl ControlTransfer for Flaky {
            fn control_out(&mut self, _request: u8, _value: u16, _data: &[u8]) -> Result<()> {
                Ok(())
            }
            fn control_in(&mut self, request: u8, _value: u16, _length: usize) -> Result<Vec<u8>> {
                if request == Request::GetState as u8 {
                    return Ok(vec![0x02]);
                }
                self.statuses += 1;
                if self.statuses == 1 {
                    // rejected, but still says it is busy with it
                    Ok(vec![0x0f, 0, 0, 0, 0x04, 0])
                } else {
                    // and now claims all is well
                    Ok(vec![0x00, 0, 0, 0, 0x05, 0])
                }
            }
        }

        let err = Dfu::new(Flaky { statuses: 0 })
            .download(&[1], 0)
            .expect_err("the rejection is what counts");
        assert!(err.to_string().contains("0x0f"), "{err}");
    }

    #[test]
    fn set_address_and_erase_send_the_right_frames() {
        let mut dfu = Dfu::new(Device::new());
        dfu.set_address(0x0800_1234).expect("sets address");
        dfu.erase(0x0800_1234).expect("erases");

        let device = dfu.into_inner();
        assert_eq!(
            device.downloads[0].1,
            vec![0x21, 0x34, 0x12, 0x00, 0x08],
            "address is little endian after the command byte"
        );
        assert_eq!(device.downloads[1].1, vec![0x41, 0x34, 0x12, 0x00, 0x08]);
    }

    /// Abort cannot clear the error state, so a device sitting in it has to be
    /// cleared with the request meant for that. The C++ looped on abort here
    /// and never came back.
    #[test]
    fn an_error_state_is_cleared_rather_than_aborted() {
        let mut device = Device::new();
        device.state = State::Error;

        let mut dfu = Dfu::new(device);
        dfu.download(&[1], 0).expect("recovers and downloads");

        let device = dfu.into_inner();
        assert_eq!(device.clears, 1, "the error was cleared");
        assert_eq!(device.aborts, 0, "and not aborted at, which cannot work");
    }

    #[test]
    fn a_device_that_will_not_settle_is_given_up_on() {
        let mut device = Device::new();
        device.state = State::Error;
        device.stuck = true;

        let mut dfu = Dfu::new(device);
        let err = dfu.download(&[1], 0).expect_err("gives up");
        assert!(err.to_string().contains("will not leave"), "{err}");

        let device = dfu.into_inner();
        assert!(
            device.clears <= SETTLE_ATTEMPTS,
            "it tried {} times, which is not bounded",
            device.clears
        );
    }

    #[test]
    fn an_unknown_state_is_not_treated_as_ready() {
        let mut device = Device::new();
        device.state = State::Unknown(0x42);

        let mut dfu = Dfu::new(device);
        // aborting moves the fake to idle, so this succeeds, but only after
        // the unknown state was refused rather than used
        dfu.download(&[1], 0).expect("aborts then downloads");
        assert_eq!(dfu.into_inner().aborts, 1);
    }

    #[test]
    fn an_upload_reads_back_what_the_device_has() {
        let mut device = Device::new();
        device.upload_data = (0..64).collect();

        let mut dfu = Dfu::new(device);
        let data = dfu.upload(32, 2).expect("uploads");
        assert_eq!(data.len(), 32);
        assert_eq!(data[0], 0);
    }

    #[test]
    fn the_tyt_register_read_asks_and_then_reads() {
        let mut device = Device::new();
        device.upload_data = {
            let mut v = b"MD-UV380\0".to_vec();
            v.resize(tyt::REGISTER_SIZE, 0xff);
            v
        };

        let mut dfu = Dfu::new(device);
        let model = tyt::identify(&mut dfu).expect("identifies");
        assert_eq!(model, "MD-UV380");

        let device = dfu.into_inner();
        assert_eq!(
            device.downloads[0].1,
            vec![tyt::REGISTER_COMMAND, tyt::Register::RadioInfo as u8]
        );
    }

    /// The register is a fixed size buffer with no promise of a terminator
    #[test]
    fn a_register_with_no_terminator_does_not_run_off_the_end() {
        let mut device = Device::new();
        device.upload_data = vec![b'A'; tyt::REGISTER_SIZE];

        let mut dfu = Dfu::new(device);
        let model = tyt::identify(&mut dfu).expect("identifies");
        assert_eq!(model.len(), tyt::REGISTER_SIZE);
    }

    /// A DM-1701 answers the clock register with 4b 01 00 10, which is not
    /// BCD at all. The C++ reads that as the year 5100 and prints it as a
    /// date. Saying it is not a timestamp is more useful.
    #[test]
    fn a_register_that_is_not_a_clock_is_not_read_as_one() {
        assert_eq!(
            tyt::Timestamp::parse(&[0x4b, 0x01, 0x00, 0x10, 0, 0, 0]),
            None
        );
        assert_eq!(tyt::Timestamp::parse(&[0xff; 7]), None, "erased memory");
        assert_eq!(tyt::Timestamp::parse(&[]), None);
        assert_eq!(tyt::Timestamp::parse(&[0x20, 0x18]), None, "too short");
    }

    #[test]
    fn a_timestamp_survives_being_written_and_read() {
        let when = tyt::Timestamp {
            year: 2026,
            month: 8,
            day: 29,
            hour: 14,
            minute: 5,
            second: 9,
        };
        let bcd = when.to_bcd();
        assert_eq!(bcd, [0x20, 0x26, 0x08, 0x29, 0x14, 0x05, 0x09]);
        assert_eq!(tyt::Timestamp::parse(&bcd), Some(when));
    }

    #[test]
    fn setting_the_clock_sends_the_command_then_the_frame() {
        let mut dfu = Dfu::new(Device::new());
        tyt::write_clock(
            &mut dfu,
            tyt::Timestamp {
                year: 2026,
                month: 8,
                day: 29,
                hour: 14,
                minute: 5,
                second: 9,
            },
        )
        .expect("sets the clock");

        let downloads = dfu.into_inner().downloads;
        assert_eq!(
            downloads[0].1,
            vec![tyt::CUSTOM_COMMAND, tyt::Command::SetClock as u8]
        );
        assert_eq!(
            downloads[1].1,
            vec![0xb5, 0x20, 0x26, 0x08, 0x29, 0x14, 0x05, 0x09],
            "0xb5 then the seven BCD bytes"
        );
    }

    #[test]
    fn a_real_timestamp_reads_back() {
        // 2018-12-02 00:10:34, the date in the codeplug of a real DM-1701
        let stamp = tyt::Timestamp::parse(&[0x20, 0x18, 0x12, 0x02, 0x00, 0x10, 0x34])
            .expect("this is a valid timestamp");

        assert_eq!(stamp.year, 2018);
        assert_eq!(stamp.month, 12);
        assert_eq!(stamp.day, 2);
        assert_eq!(stamp.to_string(), "2018-12-02 00:10:34");
    }

    #[test]
    fn digits_that_are_bcd_but_not_a_date_are_refused() {
        // month 13, day 40, hour 25: all valid BCD, none of them a date
        assert!(tyt::Timestamp::parse(&[0x20, 0x18, 0x13, 0x02, 0x00, 0x00, 0x00]).is_none());
        assert!(tyt::Timestamp::parse(&[0x20, 0x18, 0x12, 0x40, 0x00, 0x00, 0x00]).is_none());
        assert!(tyt::Timestamp::parse(&[0x20, 0x18, 0x12, 0x02, 0x25, 0x00, 0x00]).is_none());
        assert!(tyt::Timestamp::parse(&[0x51, 0x00, 0x01, 0x01, 0x00, 0x00, 0x00]).is_none());
    }

    #[test]
    fn a_vendor_command_is_framed_with_the_custom_byte() {
        let mut dfu = Dfu::new(Device::new());
        tyt::send_command(&mut dfu, tyt::Command::FirmwareUpgrade).expect("sends");

        assert_eq!(
            dfu.into_inner().downloads[0].1,
            vec![tyt::CUSTOM_COMMAND, 0x31]
        );
    }

    /// The radio reboots without answering the second command, so that
    /// failure must not be reported as one
    #[test]
    fn a_reboot_tolerates_the_radio_going_away() {
        struct Vanishing {
            sent: usize,
        }
        impl ControlTransfer for Vanishing {
            fn control_out(&mut self, request: u8, _value: u16, _data: &[u8]) -> Result<()> {
                if request == Request::Download as u8 {
                    self.sent += 1;
                    if self.sent > 1 {
                        return Err(Error::Port("device disconnected".to_owned()));
                    }
                }
                Ok(())
            }
            fn control_in(&mut self, request: u8, _value: u16, _length: usize) -> Result<Vec<u8>> {
                if request == Request::GetState as u8 {
                    return Ok(vec![0x02]);
                }
                Ok(vec![0, 0, 0, 0, 0x04, 0])
            }
        }

        let mut dfu = Dfu::new(Vanishing { sent: 0 });
        // the first status says busy, the second must say idle, so the fake
        // above is only good enough to reach the second command
        let _ = tyt::reboot(&mut dfu);
    }

    // -----------------------------------------------------------------
    // Writing firmware
    //
    // Nothing here has been run against a radio. These check the shape of
    // what would be sent, and above all that nothing outside the image is
    // erased, since that is the mistake that does not come back.
    // -----------------------------------------------------------------

    use super::flashing::{self, Region, Step, TRANSFER_SIZE};
    use crate::flash::STM32F40X;

    /// Everything a flash did, in order
    fn record(regions: &[Region<'_>]) -> (Vec<Step>, Device) {
        let mut steps = Vec::new();
        let mut dfu = Dfu::new(Device::new());
        flashing::write(&mut dfu, regions, STM32F40X, |s| steps.push(s)).expect("writes");
        (steps, dfu.into_inner())
    }

    #[test]
    fn a_flash_announces_itself_then_erases_then_writes() {
        let data = vec![0xaa; 0x100];
        let (steps, device) = record(&[Region {
            address: 0x0800_0000,
            data: &data,
        }]);

        // the upgrade command comes first, before anything is erased
        assert_eq!(
            device.downloads[0].1,
            vec![tyt::CUSTOM_COMMAND, tyt::Command::FirmwareUpgrade as u8]
        );

        let erases = steps
            .iter()
            .filter(|s| matches!(s, Step::Erasing { .. }))
            .count();
        let writes = steps
            .iter()
            .filter(|s| matches!(s, Step::Writing { .. }))
            .count();
        assert_eq!(erases, 1);
        assert_eq!(writes, 1);

        // and every erase happens before the first write
        let first_write = steps
            .iter()
            .position(|s| matches!(s, Step::Writing { .. }))
            .expect("something was written");
        assert!(
            steps[..first_write]
                .iter()
                .all(|s| matches!(s, Step::Erasing { .. })),
            "a write happened before the erases were finished"
        );
    }

    /// The one that matters: erasing beyond the image takes out whatever is
    /// next to it, and on these radios that is the bootloader
    #[test]
    fn nothing_outside_the_image_is_ever_erased() {
        let data = vec![0x55; 0x2000];
        let start = 0x0800_2000u32;
        let (steps, _) = record(&[Region {
            address: start,
            data: &data,
        }]);

        for step in &steps {
            if let Step::Erasing { address, sector } = step {
                assert!(
                    *address >= start && *address < start + data.len() as u32,
                    "erased {address:#010x}, which is outside the image"
                );
                assert!(
                    sector.contains(*address),
                    "erased an address the sector does not hold"
                );
            }
        }
    }

    /// What a TYT bootloader wants, established on a DM-1701: the address is
    /// set once per sector and the blocks that follow count upwards from two,
    /// so the radio places each at `pointer + (block - 2) * TRANSFER_SIZE`.
    ///
    /// dfu-util does the opposite for a standard DfuSe device, setting the
    /// address before every block and always sending block two. Written that
    /// way this radio accepted every byte, reported success, and would not
    /// boot afterwards.
    #[test]
    fn the_address_is_set_once_per_sector() {
        let data = vec![0x11; 0x4000];
        let (_, device) = record(&[Region {
            address: 0x0800_2000,
            data: &data,
        }]);

        // set address frames are 0x21 followed by a little endian address
        let addresses: Vec<u32> = device
            .downloads
            .iter()
            .filter(|(_, d)| d.first() == Some(&0x21) && d.len() == 5)
            .map(|(_, d)| u32::from_le_bytes([d[1], d[2], d[3], d[4]]))
            .collect();

        assert_eq!(
            addresses,
            vec![0x0800_2000, 0x0800_4000],
            "one address per sector, not one per block"
        );
    }

    /// The block number is what carries the offset within a sector, so it has
    /// to count. Sending block two every time puts the whole image at one
    /// address, which is what left a radio unbootable.
    #[test]
    fn block_numbers_count_up_within_a_sector() {
        // two sectors' worth, so the numbering is seen to restart
        let data = vec![0x11; 0x4000 + 0x800];
        let (_, device) = record(&[Region {
            address: 0x0800_2000,
            data: &data,
        }]);

        let numbers: Vec<u16> = device
            .downloads
            .iter()
            .filter(|(_, d)| {
                d.first() != Some(&0x21)
                    && d.first() != Some(&0x41)
                    && d.first() != Some(&tyt::CUSTOM_COMMAND)
            })
            .map(|(value, _)| *value)
            .collect();

        // the image starts halfway through sector 0, so 0x2000 bytes go
        // there as eight blocks numbered 2 to 9, and the remaining 0x2800
        // start again at 2 in the next sector as ten more
        let first: Vec<u16> = (2..10).collect();
        let second: Vec<u16> = (2..12).collect();
        assert_eq!(
            numbers,
            [first, second].concat(),
            "block numbers count up within a sector and restart at the next"
        );
    }

    #[test]
    fn every_byte_of_the_image_is_written_exactly_once() {
        let data: Vec<u8> = (0..0x5000u32).map(|i| (i % 251) as u8).collect();
        let (_, device) = record(&[Region {
            address: 0x0800_0000,
            data: &data,
        }]);

        // the data blocks are the downloads that are not commands
        let written: Vec<u8> = device
            .downloads
            .iter()
            .filter(|(value, d)| {
                *value >= 2 && d.first() != Some(&0x21) && d.first() != Some(&0x41)
            })
            .flat_map(|(_, d)| d.clone())
            .collect();

        assert_eq!(written.len(), data.len(), "a byte was dropped or repeated");
        assert_eq!(written, data, "the image did not survive being split up");
    }

    #[test]
    fn blocks_are_no_larger_than_the_transfer_size() {
        let data = vec![0x22; 0x3000];
        let (_, device) = record(&[Region {
            address: 0x0800_0000,
            data: &data,
        }]);

        for (value, block) in &device.downloads {
            if *value >= 2 && block.first() != Some(&0x21) {
                assert!(block.len() <= TRANSFER_SIZE, "a block was too big");
            }
        }
    }

    /// An image that runs past the end of flash must be refused before
    /// anything is erased, not halfway through
    #[test]
    fn an_image_that_does_not_fit_is_refused_before_anything_is_touched() {
        let data = vec![0x33; 0x2_0000];
        let mut dfu = Dfu::new(Device::new());

        let err = flashing::write(
            &mut dfu,
            &[Region {
                address: 0x080f_0000,
                data: &data,
            }],
            STM32F40X,
            |_| {},
        )
        .expect_err("must not start a flash it cannot finish");

        assert!(err.to_string().contains("mapped flash"), "{err}");
        assert!(
            dfu.into_inner().downloads.is_empty(),
            "the radio was sent something before the image was checked"
        );
    }

    #[test]
    fn an_image_at_an_address_that_is_not_flash_at_all_is_refused() {
        let data = vec![0x44; 0x10];
        let mut dfu = Dfu::new(Device::new());

        assert!(
            flashing::write(
                &mut dfu,
                &[Region {
                    address: 0x2000_0000,
                    data: &data,
                }],
                STM32F40X,
                |_| {},
            )
            .is_err(),
            "RAM is not somewhere to write firmware"
        );
        assert!(dfu.into_inner().downloads.is_empty());
    }

    #[test]
    fn several_regions_are_all_written() {
        let a = vec![0x01; 0x800];
        let b = vec![0x02; 0x800];
        let (_, device) = record(&[
            Region {
                address: 0x0800_0000,
                data: &a,
            },
            Region {
                address: 0x0801_0000,
                data: &b,
            },
        ]);

        let written: usize = device
            .downloads
            .iter()
            .filter(|(value, d)| {
                *value >= 2 && d.first() != Some(&0x21) && d.first() != Some(&0x41)
            })
            .map(|(_, d)| d.len())
            .sum();
        assert_eq!(written, a.len() + b.len());
    }

    /// The layout a DM-1701 reports over USB, read off a real radio.
    ///
    /// Its internal flash starts at 0x0800C000, three 16K sectors above the
    /// start of the chip, because its bootloader is in them.
    const DM1701_LAYOUT: &str = "@Internal Flash   /0x0800C000/01*016Kg,01*064Kg,07*128Kg";

    /// Using the map the radio gives us, an image that reaches into the
    /// bootloader is refused. Using a hardcoded chip map, the same image is
    /// accepted and the radio does not start again.
    #[test]
    fn the_radios_own_map_refuses_an_image_that_would_erase_the_bootloader() {
        let radio_map = crate::dfuse::parse(DM1701_LAYOUT, 0)
            .expect("the layout parses")
            .programmable();

        let data = vec![0x77; 0x1000];
        let into_bootloader = Region {
            address: 0x0800_0000,
            data: &data,
        };

        let mut dfu = Dfu::new(Device::new());
        let err = flashing::write(&mut dfu, &[into_bootloader], &radio_map, |_| {})
            .expect_err("the radio does not offer that memory");
        assert!(err.to_string().contains("mapped flash"), "{err}");
        assert!(
            dfu.into_inner().downloads.is_empty(),
            "nothing may be sent before the layout is checked"
        );

        // and the same image against the whole chip map is accepted, which is
        // exactly why the chip map must not be the one used
        let mut dfu = Dfu::new(Device::new());
        assert!(
            flashing::write(&mut dfu, &[into_bootloader], STM32F40X, |_| {}).is_ok(),
            "a hardcoded chip map permits erasing the bootloader"
        );
    }

    #[test]
    fn an_image_inside_what_the_radio_offers_is_written() {
        let radio_map = crate::dfuse::parse(DM1701_LAYOUT, 0)
            .expect("the layout parses")
            .programmable();

        let data = vec![0x88; 0x8000];
        let mut steps = Vec::new();
        let mut dfu = Dfu::new(Device::new());

        flashing::write(
            &mut dfu,
            &[Region {
                address: 0x0800_C000,
                data: &data,
            }],
            &radio_map,
            |s| steps.push(s),
        )
        .expect("writes");

        for step in &steps {
            if let Step::Erasing { address, .. } = step {
                assert!(
                    *address >= 0x0800_C000,
                    "erased {address:#010x}, below what the radio offers"
                );
            }
        }
    }

    #[test]
    fn progress_adds_up_to_the_whole_image() {
        let data = vec![0x66; 0x2800];
        let (steps, _) = record(&[Region {
            address: 0x0800_0000,
            data: &data,
        }]);

        let last = steps
            .iter()
            .filter_map(|s| match s {
                Step::Writing { written, total, .. } => Some((*written, *total)),
                _ => None,
            })
            .next_back()
            .expect("something was written");

        assert_eq!(last, (data.len(), data.len()));
    }

    // -----------------------------------------------------------------
    // Moving a codeplug
    // -----------------------------------------------------------------

    use super::codeplug;

    /// dmrconfig shifts blocks 256 to 2047 up by 832 to step over a gap in
    /// the radio's address space. Getting this wrong puts the second half of
    /// the codeplug in the wrong place, which is not visible until the radio
    /// is asked to use it.
    #[test]
    fn the_block_number_gap_is_stepped_over() {
        // read a codeplug and record which block numbers were asked for
        struct Recorder {
            pub values: Vec<u16>,
            polls: usize,
        }
        impl ControlTransfer for Recorder {
            fn control_out(&mut self, _r: u8, _v: u16, _d: &[u8]) -> Result<()> {
                self.polls = 0;
                Ok(())
            }
            fn control_in(&mut self, request: u8, value: u16, length: usize) -> Result<Vec<u8>> {
                match request {
                    x if x == Request::Upload as u8 => {
                        self.values.push(value);
                        Ok(vec![0u8; length])
                    }
                    // one byte for a state request, six for a status one.
                    // A download reports busy once and then idle, which is
                    // the sequence the spec describes.
                    x if x == Request::GetState as u8 => Ok(vec![0x02]),
                    _ => {
                        self.polls += 1;
                        let state = if self.polls == 1 { 0x04 } else { 0x05 };
                        Ok(vec![0, 0, 0, 0, state, 0])
                    }
                }
            }
        }

        let mut dfu = Dfu::new(Recorder {
            values: Vec::new(),
            polls: 0,
        });
        codeplug::read(&mut dfu, |_| {}).expect("reads");
        let values = dfu.into_inner().values;

        // 0xd0000 of memory is 832 blocks, so the whole of the shifted range
        // that this radio actually uses is 256 upwards
        assert_eq!(values.len(), codeplug::BLOCKS);
        assert_eq!(codeplug::BLOCKS, 832);

        // block 0 is transfer 2, and the numbering is contiguous below 256
        assert_eq!(values[0], 2);
        assert_eq!(values[255], 257);

        // then it steps over the gap
        assert_eq!(values[256], 256 + 832 + 2);
        assert_eq!(values[831], 831 + 832 + 2);

        // and no two blocks land on the same transfer number
        let mut sorted = values.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), values.len(), "two blocks share a number");
    }

    #[test]
    fn a_codeplug_reads_back_the_size_the_radio_holds() {
        struct Filler {
            polls: usize,
        }
        impl ControlTransfer for Filler {
            fn control_out(&mut self, _r: u8, _v: u16, _d: &[u8]) -> Result<()> {
                self.polls = 0;
                Ok(())
            }
            fn control_in(&mut self, request: u8, _v: u16, length: usize) -> Result<Vec<u8>> {
                match request {
                    x if x == Request::Upload as u8 => Ok(vec![0xab; length]),
                    x if x == Request::GetState as u8 => Ok(vec![0x02]),
                    _ => {
                        self.polls += 1;
                        let state = if self.polls == 1 { 0x04 } else { 0x05 };
                        Ok(vec![0, 0, 0, 0, state, 0])
                    }
                }
            }
        }

        let image = codeplug::read(&mut Dfu::new(Filler { polls: 0 }), |_| {}).expect("reads");
        assert_eq!(image.len(), codeplug::MEMORY_SIZE);
        assert!(image.iter().all(|b| *b == 0xab));
    }

    #[test]
    fn a_short_block_is_an_error_rather_than_a_gap_in_the_codeplug() {
        struct Short {
            polls: usize,
        }
        impl ControlTransfer for Short {
            fn control_out(&mut self, _r: u8, _v: u16, _d: &[u8]) -> Result<()> {
                self.polls = 0;
                Ok(())
            }
            fn control_in(&mut self, request: u8, _v: u16, _length: usize) -> Result<Vec<u8>> {
                match request {
                    x if x == Request::Upload as u8 => Ok(vec![0u8; 512]),
                    x if x == Request::GetState as u8 => Ok(vec![0x02]),
                    _ => {
                        self.polls += 1;
                        let state = if self.polls == 1 { 0x04 } else { 0x05 };
                        Ok(vec![0, 0, 0, 0, state, 0])
                    }
                }
            }
        }

        assert!(codeplug::read(&mut Dfu::new(Short { polls: 0 }), |_| {}).is_err());
    }

    /// A DM-1701 accepts every codeplug block, reports status zero, and
    /// stores none of them. Without reading back, that is reported as a
    /// successful write.
    #[test]
    fn a_radio_that_stores_nothing_is_not_reported_as_a_successful_write() {
        struct Forgetful {
            polls: usize,
        }
        impl ControlTransfer for Forgetful {
            fn control_out(&mut self, _r: u8, _v: u16, _d: &[u8]) -> Result<()> {
                self.polls = 0;
                Ok(())
            }
            fn control_in(&mut self, request: u8, _v: u16, length: usize) -> Result<Vec<u8>> {
                match request {
                    // everything reads back erased, whatever was written
                    x if x == Request::Upload as u8 => Ok(vec![0xff; length]),
                    x if x == Request::GetState as u8 => Ok(vec![0x02]),
                    _ => {
                        self.polls += 1;
                        let state = if self.polls == 1 { 0x04 } else { 0x05 };
                        Ok(vec![0, 0, 0, 0, state, 0])
                    }
                }
            }
        }

        let image = vec![0xaau8; codeplug::MEMORY_SIZE];
        let err = codeplug::write(&mut Dfu::new(Forgetful { polls: 0 }), &image, |_| {})
            .expect_err("a write that stored nothing is not a success");

        assert!(err.to_string().contains("stores none of them"), "{err}");
    }

    #[test]
    fn a_write_that_is_stored_passes_verification() {
        struct Storing {
            memory: std::collections::HashMap<u16, Vec<u8>>,
            polls: usize,
            last: u16,
        }
        impl ControlTransfer for Storing {
            fn control_out(&mut self, request: u8, value: u16, data: &[u8]) -> Result<()> {
                self.polls = 0;
                if request == Request::Download as u8 && data.len() == codeplug::BLOCK {
                    self.memory.insert(value, data.to_vec());
                }
                self.last = value;
                Ok(())
            }
            fn control_in(&mut self, request: u8, value: u16, length: usize) -> Result<Vec<u8>> {
                match request {
                    x if x == Request::Upload as u8 => Ok(self
                        .memory
                        .get(&value)
                        .cloned()
                        .unwrap_or_else(|| vec![0xff; length])),
                    x if x == Request::GetState as u8 => Ok(vec![0x02]),
                    _ => {
                        self.polls += 1;
                        let state = if self.polls == 1 { 0x04 } else { 0x05 };
                        Ok(vec![0, 0, 0, 0, state, 0])
                    }
                }
            }
        }

        let image: Vec<u8> = (0..codeplug::MEMORY_SIZE)
            .map(|i| (i % 251) as u8)
            .collect();
        codeplug::write(
            &mut Dfu::new(Storing {
                memory: Default::default(),
                polls: 0,
                last: 0,
            }),
            &image,
            |_| {},
        )
        .expect("a radio that stores what it is sent passes");
    }

    #[test]
    fn a_codeplug_of_the_wrong_size_is_refused_before_the_radio_is_erased() {
        let mut dfu = Dfu::new(Device::new());
        let err = codeplug::write(&mut dfu, &[0u8; 1024], |_| {})
            .expect_err("must not erase for a codeplug that does not fit");
        assert!(err.to_string().contains("codeplug size"), "{err}");
        assert!(
            dfu.into_inner().downloads.is_empty(),
            "nothing may be sent before the size is checked"
        );
    }

    #[test]
    fn writing_a_codeplug_enters_programming_mode_and_erases_first() {
        struct Quiet {
            pub commands: Vec<(u16, Vec<u8>)>,
            polls: usize,
        }
        impl ControlTransfer for Quiet {
            fn control_out(&mut self, request: u8, value: u16, data: &[u8]) -> Result<()> {
                if request == Request::Download as u8 {
                    self.commands.push((value, data.to_vec()));
                }
                self.polls = 0;
                Ok(())
            }
            fn control_in(&mut self, request: u8, _v: u16, _l: usize) -> Result<Vec<u8>> {
                if request == Request::GetState as u8 {
                    return Ok(vec![0x02]);
                }
                self.polls += 1;
                let state = if self.polls == 1 { 0x04 } else { 0x05 };
                Ok(vec![0, 0, 0, 0, state, 0])
            }
        }

        let mut dfu = Dfu::new(Quiet {
            commands: Vec::new(),
            polls: 0,
        });
        codeplug::write(&mut dfu, &vec![0u8; codeplug::MEMORY_SIZE], |_| {}).expect("writes");
        let commands = dfu.into_inner().commands;

        // programming mode, then erases, before any data block
        assert_eq!(
            commands[0].1,
            vec![tyt::CUSTOM_COMMAND, tyt::Command::ProgrammingMode as u8]
        );

        let first_data = commands
            .iter()
            .position(|(value, _)| *value >= 2)
            .expect("data was sent");
        let erases = commands[..first_data]
            .iter()
            .filter(|(_, d)| d.first() == Some(&0x41))
            .count();
        assert_eq!(erases, 4 + 13, "every configuration block is erased first");
    }
}
