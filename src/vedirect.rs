use core::array::TryFromSliceError;
use core::convert::TryFrom;
use num_enum::{TryFromPrimitive, TryFromPrimitiveError};
use thiserror::Error;

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, TryFromPrimitive)]
pub enum CommandId {
    Boot = 0,
    Ping = 1,
    Version = 3,
    Product = 4,
    Restart = 6,
    Get = 7,
    Set = 8,
    Async = 0xA,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, TryFromPrimitive)]
pub enum ResponseId {
    Done = 1,
    Unknown = 3,
    Error = 4,
    Ping = 5,
    Get = 7,
    Set = 8,
    Async = 0xA,
}

bitflags::bitflags! {
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Flags: u8 {
    const UnknownId = 0x01;
    const NotSupported = 0x02;
    const ParameterError = 0x04;
}
}

#[repr(u16)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, TryFromPrimitive)]
pub enum ErrorId {
    Checksum = 0xAAAA,
    Boot = 0,
}

#[repr(u16)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, TryFromPrimitive)]
pub enum ProductId {
    BlueSolarMppt70v15a = 0x0300,
    SmartSolarMppt100v20a = 0xa066,
}

#[repr(u16)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, TryFromPrimitive)]
pub enum ItemId {
    Product = 0x0100,
    Group = 0x0104,
    Serial = 0x010a,
    Model = 0x010b,
    Capabilities = 0x0140,
    Mode = 0x0200,
    State = 0x0201,
    Remote = 0x0202,
    OffReason1 = 0x0205,
    OffReason2 = 0x0207,
    TotalChargeCurrent = 0x2013,
    BatteryTemperature = 0xedec,
    SystemYield = 0xeddd,
    ChargerTemperature = 0xeddb,
    ChargerCurrent = 0xedd7,
    ChargerVoltage = 0xedd5,
    PanelPower = 0xedbc,
    PanelVoltage = 0xedbb,
    PanelCurrent = 0xedbd,
    LoadCurrent = 0xedad,
    BatteryMaximumCurrent = 0xedf0,
}

#[derive(Error, Debug)]
pub enum VeDirectError {
    #[error("Invalid hex data")]
    Hex,
    #[error("Too mich or too little data")]
    Length,
    #[error("Invalid of missing checksum")]
    Checksum,
    #[error("Invalid response ID")]
    Response(#[from] TryFromPrimitiveError<ResponseId>),
    #[error("Invalid response ID")]
    Error(#[from] TryFromPrimitiveError<ErrorId>),
    #[error("Slice length")]
    Slice(#[from] TryFromSliceError),
    #[error("Flags")]
    Flags,
    #[error("Item Id")]
    Item(#[from] TryFromPrimitiveError<ItemId>),
}

fn nibble(c: u8) -> Result<u8, VeDirectError> {
    if c.is_ascii_digit() {
        Ok(c - b'0')
    } else if (b'A'..=b'F').contains(&c) {
        Ok(c - b'A' + 10)
    } else if (b'a'..=b'f').contains(&c) {
        Ok(c - b'a' + 10)
    } else {
        Err(VeDirectError::Hex)
    }
}

fn hex(c: u8) -> Result<u8, VeDirectError> {
    if (0x0..=0x9).contains(&c) {
        Ok(b'0' + c)
    } else if (0xA..=0xF).contains(&c) {
        Ok(b'A' + (c - 10))
    } else {
        Err(VeDirectError::Hex)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
enum State {
    #[default]
    Start,
    Low,
    High,
}

#[derive(Default, Clone, Eq, PartialEq, Debug)]
pub struct Frame {
    data: Vec<u8>,
}

impl Frame {
    pub fn checksum(&self) -> u8 {
        self.data.iter().fold(0u8, |a, &e| a.wrapping_add(e))
    }

    pub fn valid(&self) -> bool {
        self.checksum() == 0x55
    }

    pub fn de(&mut self) -> FrameDe {
        FrameDe {
            frame: self,
            state: State::Start,
        }
    }

    pub fn ser(&self) -> FrameSer {
        FrameSer {
            frame: self,
            pos: 0,
            state: State::Start,
        }
    }
}

#[derive(Eq, PartialEq, Debug)]
pub struct FrameDe<'a> {
    frame: &'a mut Frame,
    state: State,
}

#[derive(Eq, PartialEq, Debug)]
pub struct FrameSer<'a> {
    frame: &'a Frame,
    pos: usize,
    state: State,
}

impl<'a> FrameDe<'a> {
    pub fn push(&mut self, c: u8) -> Result<(), VeDirectError> {
        match self.state {
            State::Start => {
                if c == b':' {
                    self.frame.data.clear();
                    self.frame.data.push(0);
                    self.state = State::Low;
                }
            }
            State::Low => {
                let x = self.frame.data.last_mut().unwrap();
                *x |= nibble(c)?;
                self.state = State::High;
            }
            State::High => {
                if c == b'\n' {
                    self.state = State::Start;
                } else {
                    self.frame.data.push(nibble(c)? << 4);
                    self.state = State::Low;
                }
            }
        };
        Ok(())
    }

    pub fn push_slice(&mut self, value: &[u8]) -> Result<(), VeDirectError> {
        for c in value.iter() {
            self.push(*c)?;
        }
        Ok(())
    }

    pub fn done(&self) -> bool {
        self.state == State::Start && !self.frame.data.is_empty()
    }
}

impl<'a> Iterator for FrameSer<'a> {
    type Item = u8;

    fn next(&mut self) -> Option<u8> {
        Some(match self.state {
            State::Start => {
                if self.pos > 0 {
                    return None;
                }
                self.state = State::Low;
                b':'
            }
            State::Low => {
                self.state = State::High;
                self.pos += 1;
                hex(self.frame.data[self.pos - 1] & 0xf).unwrap()
            }
            State::High => {
                if self.pos == self.frame.data.len() {
                    self.state = State::Start;
                    b'\n'
                } else {
                    self.state = State::Low;
                    hex(self.frame.data[self.pos] >> 4).unwrap()
                }
            }
        })
    }
}

impl From<&Frame> for Vec<u8> {
    fn from(value: &Frame) -> Self {
        let s = value.ser();
        let mut v = Self::new();
        for c in s {
            v.push(c);
        }
        v
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Response<'a> {
    Done(&'a [u8]),
    Unknown(&'a [u8]),
    Error(ErrorId),
    Ping {
        flags: u8,
        major: u8,
        minor: u8,
    },
    Update {
        item: ItemId,
        flags: Flags,
        value: &'a [u8],
    },
}

fn bcd_to_bin(c: u8) -> u8 {
    (c & 0xf) + 10 * (c >> 4)
}

impl<'a> TryFrom<&'a Frame> for Response<'a> {
    type Error = VeDirectError;

    fn try_from(frame: &'a Frame) -> Result<Self, VeDirectError> {
        let data = &frame.data[1..frame.data.len() - 1];
        if !frame.valid() {
            return Err(VeDirectError::Checksum);
        }
        Ok(match ResponseId::try_from(frame.data[0])? {
            ResponseId::Done => Self::Done(data),
            ResponseId::Unknown => Self::Unknown(data),
            ResponseId::Error => Self::Error(u16::from_le_bytes(data[..2].try_into()?).try_into()?),
            ResponseId::Ping => Self::Ping {
                flags: data[1] >> 4,
                major: data[1] & 0xf,
                minor: bcd_to_bin(data[0]),
            },
            ResponseId::Get | ResponseId::Set | ResponseId::Async => Self::Update {
                item: u16::from_le_bytes(data[..2].try_into()?).try_into()?,
                flags: Flags::from_bits(data[2]).ok_or(VeDirectError::Flags)?,
                value: &data[3..],
            },
        })
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Command<'a> {
    Boot,
    Ping,
    Version,
    Product,
    Restart,
    Get {
        item: ItemId,
        flags: Flags,
    },
    Set {
        item: ItemId,
        flags: Flags,
        value: &'a [u8],
    },
    Async {
        item: ItemId,
        flags: Flags,
        value: &'a [u8],
    },
}

impl<'a> Command<'a> {
    //pub fn get(i: ItemId) -> Command<'a> {}
}

impl<'a> From<&'a Command<'a>> for Frame {
    fn from(value: &'a Command) -> Self {
        let mut f = Frame::default();
        f.data.push(match &value {
            Command::Boot => CommandId::Boot,
            Command::Ping => CommandId::Ping,
            Command::Version => CommandId::Version,
            Command::Product => CommandId::Product,
            Command::Restart => CommandId::Restart,
            Command::Get { .. } => CommandId::Get,
            Command::Set { .. } => CommandId::Set,
            Command::Async { .. } => CommandId::Async,
        } as _);
        match &value {
            Command::Get { item, flags } => {
                f.data.extend_from_slice(&(*item as u16).to_le_bytes());
                f.data.push(flags.bits());
            }
            Command::Set { item, flags, value } => {
                f.data.extend_from_slice(&(*item as u16).to_le_bytes());
                f.data.push(flags.bits());
                f.data.extend_from_slice(value);
            }
            Command::Async { item, flags, value } => {
                f.data.extend_from_slice(&(*item as u16).to_le_bytes());
                f.data.push(flags.bits());
                f.data.extend_from_slice(value);
            }
            _ => {}
        }
        let check = f.data.iter().fold(0x55u8, |a, e| a.wrapping_sub(*e));
        f.data.push(check);
        f
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLES: [(Command, &[u8], &[u8]); 7] = [
        (Command::Ping, b":154\n", b":51641F9\n"),
        (Command::Version, b":352\n", b":11641FD\n"),
        (Command::Product, b":451\n", b":1000351\n"),
        (
            Command::Get {
                item: ItemId::BatteryMaximumCurrent,
                flags: Flags::empty(),
            },
            b":7F0ED0071\n",
            b":7F0ED009600DB\n",
        ),
        (
            Command::Set {
                item: ItemId::BatteryMaximumCurrent,
                flags: Flags::empty(),
                value: &100u16.to_le_bytes(),
            },
            b":8F0ED0064000C\n",
            b":8F0ED0064000C\n",
        ),
        (Command::Ping, b":253\n", b":3020050\n"),
        //(b":452\n", b":4AAAAFD\n"),
        (Command::Restart, b":64F\n", b":A0102000543\n"),
    ];

    #[test]
    fn de() {
        for (_, cmd, resp) in EXAMPLES.iter() {
            let mut f = Frame::default();
            assert!(f.push_slice(*cmd).unwrap());
            println!("{:?}", f);
            assert!(f.push_slice(*resp).unwrap());
            println!("{:?}", f);
            let r = Response::try_from(&f).unwrap();
            println!("{:?}", r);
        }
    }

    #[test]
    fn ser() {
        for (cmd, _, _) in EXAMPLES.iter() {
            let mut f = Frame::from(cmd);
            println!("{:?}", f);
            let v: Vec<u8> = (&mut f).try_into().unwrap();
            let s: String = v.iter().map(|c| *c as char).collect();
            println!("{:?}", s);
        }
    }
}
