use rosserial_rs::{RosSerial, RosSerialMsg};
use std::error::Error;
use tokio_serial::SerialPortBuilderExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();
    
    let pub_topics_handler_fn = async |topic: &str, message_type: &str, msg: Vec<u8>| -> rosserial_rs::Result<()> {
        println!("received message on {} [{}]: {:?}", topic, message_type, msg);
        Ok(())
    };

    let port = tokio_serial::new("COM9", 115200).open_native_async()?;

    let mut rosserial = RosSerial::new(port, pub_topics_handler_fn).await?;
    rosserial.run().await?;

    Ok(())
}
