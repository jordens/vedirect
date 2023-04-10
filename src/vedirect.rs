use core::array::TryFromSliceError;
use core::convert::TryFrom;
use num_enum::{TryFromPrimitive, TryFromPrimitiveError};
use thiserror::Error;

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, TryFromPrimitive, Hash)]
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
#[derive(Clone, Copy, Debug, Eq, PartialEq, TryFromPrimitive, Hash)]
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
#[derive(Clone, Copy, Debug, Eq, PartialEq, TryFromPrimitive, Hash)]
pub enum ErrorId {
    Checksum = 0xAAAA,
    Boot = 0,
}

#[repr(u16)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, TryFromPrimitive, Hash)]
pub enum ProductId {
    BlueSolarMppt70v15a = 0x0300,
    SmartSolarMppt100v20a = 0xa066,
}

#[repr(u16)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, TryFromPrimitive, Hash)]
pub enum ItemId {
    Product = 0x0100,
    Group = 0x0104,
    Serial = 0x010a,
    Model = 0x010b,
    Unknown0x010e = 0x010e,
    Capabilities = 0x0140,
    Mode = 0x0200,
    State = 0x0201,
    Remote = 0x0202,
    OffReason1 = 0x0205,
    OffReason2 = 0x0207,
    TotalHistory = 0x104f,
    DailyHistory0 = 0x1050,
    BatteryVoltageSense = 0x2002,
    BatteryTemperatureSense = 0x2003,
    NetworkInfo = 0x200d,
    NetworkMode = 0x200e,
    TotalChargeCurrent = 0x2013,
    TotalDCInputPower = 0x2027,
    SolarActivity = 0x2030,
    TimeOfDay = 0x2031,
    Unknown0xc6a3 = 0xc6a3,
    BatteryTemperature = 0xedec,
    SystemYield = 0xeddd,
    ChargerTemperature = 0xeddb,
    ChargerCurrent = 0xedd7,
    AdditionalChargerStateInfo = 0xedd4,
    ChargerVoltage = 0xedd5,
    YieldToday = 0xedd3,
    MaximumPowerToday = 0xedd2,
    YieldYesterday = 0xedd1,
    MaximumPowerYesterday = 0xedd0,
    PanelPower = 0xedbc,
    PanelVoltage = 0xedbb,
    PanelCurrent = 0xedbd,
    LoadVoltage = 0xeda9,
    LoadCurrent = 0xedad,
    BatteryMaximumCurrent = 0xedf0,
}

#[derive(Error, Debug, Copy, Clone)]
pub enum VeDirectError {
    #[error("Invalid hex data `{0}`")]
    Hex(u8),
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
        Err(VeDirectError::Hex(c))
    }
}

fn hex(c: u8) -> Result<u8, VeDirectError> {
    if (0x0..=0x9).contains(&c) {
        Ok(b'0' + c)
    } else if (0xA..=0xF).contains(&c) {
        Ok(b'A' + (c - 10))
    } else {
        Err(VeDirectError::Hex(c))
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

    pub fn done(&self) -> bool {
        self.state == State::Start && !self.frame.data.is_empty()
    }

    /*
       pub fn read<R: std::io::Read>(&mut self, read: R) -> bool {
           let mut buf = [0];
           read.read_exact(&mut buf)?;
           Ok(self.done() && self.frame.valid())
       }
    */
}

impl TryFrom<&[u8]> for Frame {
    type Error = VeDirectError;

    fn try_from(value: &[u8]) -> Result<Frame, VeDirectError> {
        let mut f = Frame::default();
        let mut d = f.de();
        for c in value.iter() {
            d.push(*c)?;
        }
        if !d.done() {
            Err(VeDirectError::Length)
        } else if !f.valid() {
            Err(VeDirectError::Checksum)
        } else {
            Ok(f)
        }
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

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Response {
    Done(Value),
    Unknown(Value),
    Error(ErrorId),
    Ping {
        flags: u8,
        major: u8,
        minor: u8,
    },
    Update {
        item: ItemId,
        flags: Flags,
        value: Value,
    },
}

fn bcd_to_bin(c: u8) -> u8 {
    (c & 0xf) + 10 * (c >> 4)
}

impl TryFrom<&Frame> for Response {
    type Error = VeDirectError;

    fn try_from(frame: &Frame) -> Result<Self, VeDirectError> {
        let data = &frame.data[1..frame.data.len() - 1];
        if !frame.valid() {
            return Err(VeDirectError::Checksum);
        }
        Ok(match ResponseId::try_from(frame.data[0])? {
            ResponseId::Done => Self::Done(Value::guess(data)),
            ResponseId::Unknown => Self::Unknown(Value::guess(data)),
            ResponseId::Error => Self::Error(u16::from_le_bytes(data[..2].try_into()?).try_into()?),
            ResponseId::Ping => Self::Ping {
                flags: data[1] >> 4,
                major: data[1] & 0xf,
                minor: bcd_to_bin(data[0]),
            },
            ResponseId::Get | ResponseId::Set | ResponseId::Async => Self::Update {
                item: u16::from_le_bytes(data[..2].try_into()?).try_into()?,
                flags: Flags::from_bits(data[2]).ok_or(VeDirectError::Flags)?,
                value: Value::guess(&data[3..]),
            },
        })
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Command {
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
        value: Value,
    },
    Async {
        item: ItemId,
        flags: Flags,
        value: Value,
    },
}

impl Command {
    pub fn as_frame(&self) -> Frame {
        let mut f = Frame::default();
        f.data.push(match &self {
            Command::Boot => CommandId::Boot,
            Command::Ping => CommandId::Ping,
            Command::Version => CommandId::Version,
            Command::Product => CommandId::Product,
            Command::Restart => CommandId::Restart,
            Command::Get { .. } => CommandId::Get,
            Command::Set { .. } => CommandId::Set,
            Command::Async { .. } => CommandId::Async,
        } as _);
        match &self {
            Command::Get { item, flags } => {
                f.data.extend_from_slice(&(*item as u16).to_le_bytes());
                f.data.push(flags.bits());
            }
            Command::Set { item, flags, value } => {
                f.data.extend_from_slice(&(*item as u16).to_le_bytes());
                f.data.push(flags.bits());
                value.ser(&mut f.data);
            }
            Command::Async { item, flags, value } => {
                f.data.extend_from_slice(&(*item as u16).to_le_bytes());
                f.data.push(flags.bits());
                value.ser(&mut f.data);
            }
            _ => {}
        }
        let check = f.data.iter().fold(0x55u8, |a, &e| a.wrapping_sub(e));
        f.data.push(check);
        f
    }

    pub const fn get(item: ItemId) -> Command {
        Self::Get {
            item,
            flags: Flags::empty(),
        }
    }
}

impl From<&Command> for Frame {
    fn from(value: &Command) -> Self {
        value.as_frame()
    }
}

#[derive(Default, Debug, Clone, Eq, PartialEq, Hash)]
pub enum Value {
    #[default]
    Empty,
    U8(u8),
    I8(i8),
    U16(u16),
    I16(i16),
    U32(u32),
    I32(i32),
    Ascii(String),
    Other(Vec<u8>),
}

impl Value {
    pub fn guess(value: &[u8]) -> Self {
        match value.len() {
            0 => Self::Empty,
            1 => Self::U8(value[0]),
            2 => Self::U16(u16::from_le_bytes(value.try_into().unwrap())),
            4 => Self::U32(u32::from_le_bytes(value.try_into().unwrap())),
            _ => Self::Other(value.into()),
        }
    }

    fn ser(&self, vec: &mut Vec<u8>) {
        match self {
            Self::Empty => {}
            Self::U8(v) => vec.push(*v),
            Self::I8(v) => vec.push(*v as _),
            Self::U16(v) => vec.extend(v.to_le_bytes()),
            Self::I16(v) => vec.extend(v.to_le_bytes()),
            Self::U32(v) => vec.extend(v.to_le_bytes()),
            Self::I32(v) => vec.extend(v.to_le_bytes()),
            Self::Ascii(v) => vec.extend(v.as_bytes()),
            Self::Other(v) => vec.extend(v),
        }
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
                value: Value::U16(100),
            },
            b":8F0ED0064000C\n",
            b":8F0ED0064000C\n",
        ),
        (Command::Ping, b":253\n", b":3020050\n"),
        //(b":452\n", b":4AAAAFD\n"),
        (Command::Restart, b":64F\n", b":A0102000543\n"),
    ];

    #[test]
    fn serde() {
        for (cmd, req, resp) in EXAMPLES.iter() {
            let v = cmd.as_frame().ser().collect();
            println!("{cmd:?}: {:?}", String::from_utf8(v).unwrap());

            let f = Frame::try_from(*req).unwrap();
            println!("{:?}", f);

            let f = Frame::try_from(*resp).unwrap();
            let r = Response::try_from(&f).unwrap();
            println!("{f:?}: {r:?}");
        }
    }
}
