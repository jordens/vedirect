use log::{info, warn};
use std::io::prelude::*;
use std::time::{Duration, Instant};

pub mod vedirect;
use vedirect::{Command, Frame, ItemId, Response, Value};

type Cache = std::collections::HashMap<ItemId, Value>;

fn idb(msg: &Cache, station: &str) -> String {
    let mut s = String::new();
    s.push_str("vedirect,station=");
    s.push_str(station);
    s.push(' ');
    for (key, value) in msg.iter() {
        let v = value.to_string();
        if v.is_empty() {
            continue;
        }
        s.push_str(&key.to_string());
        s.push('=');
        s.push_str(&v);
        s.push(',');
    }
    s.pop();
    s
}

fn main() -> anyhow::Result<()> {
    #[cfg(debug_assertions)]
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let mut args = pico_args::Arguments::from_env();
    let mut dev = serialport::new(
        args.opt_value_from_str("--device")?
            .unwrap_or_else(|| "/dev/ttyUSB0".to_string()),
        19_200,
    )
    .data_bits(serialport::DataBits::Eight)
    .stop_bits(serialport::StopBits::One)
    .parity(serialport::Parity::None)
    .timeout(Duration::from_millis(1000))
    .open()?;

    let station = args
        .opt_value_from_str("--station")?
        .unwrap_or_else(|| "vedirect".to_string());
    let socket = std::net::UdpSocket::bind(
        args.opt_value_from_str("--bind")?
            .unwrap_or_else(|| "0.0.0.0:0".to_string()),
    )?;
    let target: Option<std::net::SocketAddr> = args.opt_value_from_str("--target")?;
    let every = args.opt_value_from_str("--every")?.unwrap_or(50);

    let mut cache = Cache::new();

    let ping = Frame::try_from(&Command::Ping)?.ser().collect::<Vec<u8>>();
    let mut next = Instant::now();

    loop {
        if next <= Instant::now() {
            if !cache.is_empty() {
                let s = idb(&cache, &station);
                println!("{}", s);
                cache.clear();
                if let Some(t) = target.as_ref() {
                    socket.send_to(s.as_bytes(), t)?;
                }
            }
            dev.write_all(&ping)?;
            next += Duration::from_secs(every);
        }

        let mut ve = Frame::default();
        {
            let mut de = ve.de();
            let mut buf = [0; 1];
            loop {
                match dev.read(&mut buf) {
                    Ok(n) => {
                        if n > 0 {
                            if let Err(e) = de.push(buf[0]) {
                                warn!("invalid data `{:?}`: {:?}", buf, e);
                            } else if de.done() {
                                break;
                            }
                        } else {
                            return Ok(()); // EOF?
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::TimedOut => continue,
                    Err(e) => Err(e)?,
                }
            }
        };
        if ve.valid() {
            match Response::try_from(&ve) {
                Ok(r) => {
                    info!("frame: {:?}", r);
                    if let Response::Update {
                        item, flags, value, ..
                    } = r
                    {
                        if flags.is_empty() {
                            cache.insert(item, value);
                        }
                    }
                }
                Err(e) => warn!("invalid frame: `{:?}`", e),
            }
        }
    }

    // Ok(())
}
