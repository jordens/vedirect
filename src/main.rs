use log::{debug, info, warn};
use std::io::prelude::*;
use std::time::{Duration, Instant};
use thiserror::Error;

pub mod vedirect;

fn main() -> anyhow::Result<()> {
    #[cfg(debug_assertions)]
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("vedirect=warn"))
        .init();

    let mut args = pico_args::Arguments::from_env();
    let mut dev = serialport::new(
        args.opt_value_from_str("--device")?
            .unwrap_or_else(|| "/dev/ttyUSB0".to_string()),
        19_200,
    )
    .data_bits(serialport::DataBits::Eight)
    .stop_bits(serialport::StopBits::One)
    .parity(serialport::Parity::None)
    .timeout(Duration::from_secs(0))
    .open()?;

    /*
    let station = args
        .opt_value_from_str("--station")?
        .unwrap_or_else(|| "vedirect".to_string());
    let socket = std::net::UdpSocket::bind(
        args.opt_value_from_str("--bind")?
            .unwrap_or_else(|| "0.0.0.0:0".to_string()),
    )?;
    let target: Option<std::net::SocketAddr> = args.opt_value_from_str("--target")?;
    let every = args.opt_value_from_str("--every")?.unwrap_or(0);
    */

    let mut cache = std::collections::HashMap::new();

    let ping: vedirect::Frame = (&vedirect::Command::Ping).try_into()?;
    dev.write_all(&ping.ser().collect::<Vec<u8>>())?;

    loop {
        let mut ve = vedirect::Frame::default();
        let valid = {
            let mut buf = [0; 1];
            let mut de = ve.de();
            loop {
                match dev.read(&mut buf) {
                    Ok(n) => {
                        if n > 0 {
                            if let Err(e) = de.push(buf[0]) {
                                println!("{:?}", e);
                            } else if de.done() {
                                break ve.valid();
                            }
                        } else {
                            return Ok(());
                        }
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => continue,
                    Err(e) => Err(e)?,
                }
            }
        };
        if valid {
            let r: Result<vedirect::Response, vedirect::VeDirectError> = (&ve).try_into();
            match r {
                Ok(r) => {
                    println!("{:?}", r);
                    if let vedirect::Response::Update { item, flags, value } = r {
                        if flags.is_empty() {
                            cache.insert(item, value);
                        }
                    }
                }
                Err(e) => println!("{:?}", e),
            }
        }
    }

    // Ok(())
}
