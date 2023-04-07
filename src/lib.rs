use core::array::TryFromSliceError;
use core::convert::TryFrom;
use heapless::Vec;
use num_enum::{TryFromPrimitive, TryFromPrimitiveError};
use thiserror::Error;

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, TryFromPrimitive)]
enum CommandId {
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
enum ResponseId {
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
struct Flags: u8 {
    const UnknownId = 0x01;
    const NotSupported = 0x02;
    const ParameterError = 0x04;
}
}

#[repr(u16)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, TryFromPrimitive)]
enum ErrorId {
    Checksum = 0xAAAA,
    Boot = 0,
}

#[repr(u16)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, TryFromPrimitive)]
enum ProductId {
    BlueSolarMppt70v15a = 0x0300,
    SmartSolarMppt100v20a = 0xa066,
}

#[repr(u16)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, TryFromPrimitive)]
enum ItemId {
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
enum VeDirectError {
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
    Id,
    High,
    Low,
}

#[derive(Default, Clone, Eq, PartialEq, Debug)]
struct Frame {
    data: Vec<u8, 64>,
    check: u8,
    id: u8,
    pending: u8,
    state: State,
    pos: usize,
}

impl Frame {
    pub fn push(&mut self, c: u8) -> Result<(), VeDirectError> {
        match self.state {
            State::Start => {
                if c == b':' {
                    self.state = State::Id;
                }
            }
            State::Id => {
                self.id = nibble(c)?;
                self.data.clear();
                self.check = self.id;
                self.state = State::High;
            }
            State::High => {
                if c == b'\n' {
                    // self.data.pop().ok_or(VeDirectError::Checksum)?;
                    self.state = State::Start;
                } else {
                    self.pending = nibble(c)? << 4;
                    self.state = State::Low;
                }
            }
            State::Low => {
                let x = self.pending | nibble(c)?;
                self.data.push(x).or(Err(VeDirectError::Length))?;
                self.check = self.check.wrapping_add(x);
                self.state = State::High;
            }
        };
        Ok(())
    }

    pub fn valid(&self) -> bool {
        self.state == State::Start && self.check == 0x55
    }

    pub fn pop(&mut self) -> Result<u8, VeDirectError> {
        Ok(match self.state {
            State::Start => {
                self.state = State::Id;
                b':'
            }
            State::Id => {
                self.pos = 0;
                self.state = State::High;
                hex(self.id)?
            }
            State::High => {
                if self.pos == self.data.len() {
                    // assert!(self.check == 0);
                    self.state = State::Start;
                    b'\n'
                } else {
                    // self.check = self.check.wrapping_sub(self.data[self.pos]);
                    self.state = State::Low;
                    hex(self.data[self.pos] >> 4)?
                }
            }
            State::Low => {
                self.state = State::High;
                self.pos += 1;
                hex(self.data[self.pos - 1] & 0xf)?
            }
        })
    }
}

impl TryFrom<&[u8]> for Frame {
    type Error = VeDirectError;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        let mut f = Frame::default();
        for c in value.iter() {
            f.push(*c)?;
        }
        Ok(f)
    }
}

impl TryFrom<&mut Frame> for Vec<u8, 64> {
    type Error = VeDirectError;

    fn try_from(value: &mut Frame) -> Result<Self, Self::Error> {
        let mut v = Self::new();
        loop {
            let c = value.pop()?;
            v.push(c).or(Err(VeDirectError::Length))?;
            if c == b'\n' {
                break;
            }
        }
        Ok(v)
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
enum Response<'a> {
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

fn u8_to_dec(c: u8) -> u8 {
    (c & 0xf) + 10 * (c >> 4)
}

impl<'a> TryFrom<&'a Frame> for Response<'a> {
    type Error = VeDirectError;

    fn try_from(frame: &'a Frame) -> Result<Self, VeDirectError> {
        let data = &frame.data[..frame.data.len() - 1];
        if !frame.valid() {
            return Err(VeDirectError::Checksum);
        }
        Ok(match ResponseId::try_from(frame.id)? {
            ResponseId::Done => Self::Done(data),
            ResponseId::Unknown => Self::Unknown(data),
            ResponseId::Error => Self::Error(u16::from_le_bytes(data[..2].try_into()?).try_into()?),
            ResponseId::Ping => Self::Ping {
                flags: data[1] >> 4,
                major: data[1] & 0xf,
                minor: u8_to_dec(data[0]),
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
enum Command<'a> {
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

impl<'a> TryFrom<&'a Command<'a>> for Frame {
    type Error = VeDirectError;

    fn try_from(value: &'a Command) -> Result<Self, VeDirectError> {
        let mut f = Frame::default();
        match value {
            Command::Boot => f.id = CommandId::Boot as _,
            Command::Ping => f.id = CommandId::Ping as _,
            Command::Version => f.id = CommandId::Version as _,
            Command::Product => f.id = CommandId::Product as _,
            Command::Restart => f.id = CommandId::Restart as _,
            Command::Get { item, flags } => {
                f.id = CommandId::Get as _;
                f.data
                    .extend_from_slice(&(*item as u16).to_le_bytes())
                    .or(Err(VeDirectError::Length))?;
                f.data.push(flags.bits()).or(Err(VeDirectError::Length))?;
            }
            Command::Set { item, flags, value } => {
                f.id = CommandId::Set as _;
                f.data
                    .extend_from_slice(&(*item as u16).to_le_bytes())
                    .or(Err(VeDirectError::Length))?;
                f.data.push(flags.bits()).or(Err(VeDirectError::Length))?;
                f.data
                    .extend_from_slice(value)
                    .or(Err(VeDirectError::Length))?;
            }
            Command::Async { item, flags, value } => {
                f.id = CommandId::Async as _;
                f.data
                    .extend_from_slice(&(*item as u16).to_le_bytes())
                    .or(Err(VeDirectError::Length))?;
                f.data.push(flags.bits()).or(Err(VeDirectError::Length))?;
                f.data
                    .extend_from_slice(value)
                    .or(Err(VeDirectError::Length))?;
            }
        }
        f.check = 0x55;
        let check = f
            .data
            .iter()
            .fold(f.check.wrapping_sub(f.id), |a, e| a.wrapping_sub(*e));
        f.data.push(check).or(Err(VeDirectError::Length))?;

        Ok(f)
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
            let f = Frame::try_from(*cmd).unwrap();
            println!("{:?}", f);
            assert!(f.valid());
            let f = Frame::try_from(*resp).unwrap();
            println!("{:?}", f);
            assert!(f.valid());
            let r = Response::try_from(&f).unwrap();
            println!("{:?}", r);
        }
    }

    #[test]
    fn ser() {
        for (cmd, _, _) in EXAMPLES.iter() {
            let mut f = Frame::try_from(cmd).unwrap();
            println!("{:?}", f);
            assert!(f.valid());
            let v: Vec<u8, 64> = (&mut f).try_into().unwrap();
            let s: String = v.iter().map(|c| *c as char).collect();
            println!("{:?}", s);
        }
    }
}
