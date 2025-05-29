use log::{error, info};
use rosserial_rs::RosSerial;
use std::env;
use std::error::Error;
use tokio::select;
use tokio_serial::SerialPortBuilderExt;
use url::Url;

const ROS_MASTER_URI: &str = "http://0.0.0.0:11311";

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    let args: Vec<String> = env::args().skip(1).collect();
    if args.len() != 1 {
        error!(
            "expected exactly one argument: the path to / identifier of the serial port, but got {} args",
            args.len()
        );
        return Err("expected exactly one argument".into());
    }
    let port = args.first().unwrap();

    // Spawn a Tokio task to run the ROS master
    let core_cancel = tokio_util::sync::CancellationToken::new();
    let t_core = tokio::spawn({
        let core_cancel = core_cancel.clone();
        async move {
            let uri = Url::parse(ROS_MASTER_URI).unwrap();
            let socket_address = ros_core_rs::url_to_socket_addr(&uri)?;
            let master = ros_core_rs::core::Master::new(&socket_address);

            select! {
                serve = master.serve() => {
                    serve
                },
                _ = core_cancel.cancelled() => {
                    Ok(())
                }
            }
        }
    });

    tokio::task::spawn_blocking(|| {
        rosrust::loop_init("rosserial_rs", 1000);
    })
    .await?;

    let port = tokio_serial::new(port, 115200).open_native_async()?;

    let mut rosserial = RosSerial::new(port).await?;

    select! {
        r = rosserial.run() => {
            if let Err(r) = r {
                error!("encountered error: {}", r);
            }
        },
        _ = tokio::signal::ctrl_c() => {},
    }

    info!("shutting down");

    // Wind down clients
    tokio::task::spawn_blocking(|| {
        rosrust::shutdown();
    })
    .await?;
    core_cancel.cancel();
    let _ = tokio::join!(t_core).0?;

    Ok(())
}
