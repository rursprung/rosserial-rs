//! Pure-Rust implementation of [rosserial](https://wiki.ros.org/rosserial).

#![forbid(unsafe_code)]
#![deny(warnings)]
#![deny(missing_docs)]
#![deny(missing_debug_implementations)]
#![deny(unused)]

mod codec;

use crate::codec::{RosSerialMsg, RosSerialMsgCodec};
use futures::SinkExt;
use log::{debug, info, warn};
use num_traits::FromPrimitive;
use std::fmt::{Display, Formatter};
use tokio_stream::StreamExt;
use tokio_util::codec::{Decoder, Framed};

/// All methods in this crate will return this kind of Result.
#[derive(Debug)]
pub enum Error {
    /// Errors from [`codec`]
    CodecError(codec::Error),
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::CodecError(_) => write!(f, "the codec encountered an error"),
        }
    }
}

impl From<codec::Error> for Error {
    fn from(e: codec::Error) -> Self {
        Error::CodecError(e)
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::CodecError(e) => Some(e),
        }
    }
}

/// All methods in this crate will return this kind of Result.
pub type Result<T> = std::result::Result<T, Error>;

pub(crate) mod rosserial_msgs {
    use num_derive::FromPrimitive;

    /// Taken from https://github.com/ros-drivers/rosserial/blob/noetic-devel/rosserial_msgs/msg/TopicInfo.msg
    #[derive(Debug, FromPrimitive)]
    pub(crate) enum TopicInfoIds {
        Publisher = 0,
        Subscriber = 1,
        ServiceServer = 2,
        ServiceClient = 4,
        ParameterRequest = 6,
        Log = 7,
        Time = 10,
        TxStop = 11,
    }
}

/// Represents a ROS Serial connection to a serial port.
//#[derive(Debug)]
#[allow(missing_debug_implementations)]
pub struct RosSerial {
    serial: Framed<tokio_serial::SerialStream, RosSerialMsgCodec>,
}

impl RosSerial {
    /// Create a new ROS Serial connection.
    pub async fn new(serial: tokio_serial::SerialStream) -> Result<Self> {
        let serial = RosSerialMsgCodec.framed(serial);
        let mut this = RosSerial { serial };
        this.request_topics().await?;
        Ok(this)
    }

    /// Run the communication
    pub async fn run(&mut self) -> Result<()> {
        while let Some(Ok(msg)) = self.serial.next().await {
            debug!("received msg in topic {} = {:?}", msg.topic, msg.msg); // TODO: trace
            self.handle_msg(msg).await?;
        }

        Ok(())
    }

    async fn handle_msg(&mut self, msg: RosSerialMsg) -> Result<()> {
        match rosserial_msgs::TopicInfoIds::from_u16(msg.topic) {
            Some(rosserial_msgs::TopicInfoIds::Time) => info!("TODO: handle TIME messages"),
            Some(t) => warn!("unimplemented core topic: {:?}", t),
            _ => warn!("received unknown topic: {}", msg.topic),
        }

        Ok(())
    }

    async fn send(&mut self, data: &[u8]) -> Result<()> {
        // TODO: this is a hack, we should use the topic here... right?
        self.serial
            .send(RosSerialMsg {
                topic: 0,
                msg: data.to_vec(),
            })
            .await?;
        Ok(())
    }

    async fn request_topics(&mut self) -> Result<()> {
        self.send(b"\x00\x00\xff\x00\x00\xff").await
    }
}
