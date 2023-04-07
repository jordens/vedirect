use log::{debug, warn};
use std::fs::File;
use std::io::prelude::*;
use std::str;
use thiserror::Error;

fn main() -> anyhow::Result<()> {
    #[cfg(debug_assertions)]
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("vedirect=warn"))
        .init();

    let mut args = pico_args::Arguments::from_env();
    let mut dev = tokio_serial::new(
        args.opt_value_from_str("--device")?
            .unwrap_or_else(|| "/dev/ttyUSB0".to_string()),
        19_200,
    ).open()?;
    
    let station = args
        .opt_value_from_str("--station")?
        .unwrap_or_else(|| "vedirect".to_string());
    let socket = std::net::UdpSocket::bind(
        args.opt_value_from_str("--bind")?
            .unwrap_or_else(|| "0.0.0.0:0".to_string()),
    )?;
    let target: Option<std::net::SocketAddr> = args.opt_value_from_str("--target")?;
    let every = args.opt_value_from_str("--every")?.unwrap_or(0);

    // let mut ve = VeDirect::new(dev);

    loop {
        // ve.ping()?;
        // info!("frame: {:?}", ve.get(Item::Product)
        // let len = dev.read(&mut buf)?;
        // debug!("frame: {:X?}", &buf[..len]);
    }
}
