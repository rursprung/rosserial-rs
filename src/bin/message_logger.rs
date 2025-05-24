use rosserial_rs::RosSerial;
use std::error::Error;
use tokio_serial::SerialPortBuilderExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    let port = tokio_serial::new("COM9", 115200).open_native_async()?;

    let mut rosserial = RosSerial::new(port).await?;
    rosserial.run().await?;

    Ok(())
}
