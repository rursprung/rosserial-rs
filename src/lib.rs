//! Pure-Rust implementation of [rosserial](https://wiki.ros.org/rosserial).

#![forbid(unsafe_code)]
#![deny(warnings)]
#![deny(missing_docs)]
#![deny(missing_debug_implementations)]
#![deny(unused)]

mod codec;

use crate::codec::{RosSerialMsg, RosSerialMsgCodec};
use futures::SinkExt;
use log::{debug, error, info, warn};
use rosrust::RosMsg;
use std::fmt::{Display, Formatter};
use tokio_stream::StreamExt;
use tokio_util::codec::{Decoder, Framed};

/// All methods in this crate will return this kind of Result.
#[derive(Debug)]
pub enum Error {
    /// Errors from [`codec`]
    CodecError(codec::Error),
    /// General IO Errors
    IoError(std::io::Error),
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::CodecError(_) => write!(f, "the codec encountered an error"),
            Error::IoError(_) => write!(f, "IO error"),
        }
    }
}

impl From<codec::Error> for Error {
    fn from(e: codec::Error) -> Self {
        Error::CodecError(e)
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::IoError(e)
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::CodecError(e) => Some(e),
            Error::IoError(e) => Some(e),
        }
    }
}

/// All methods in this crate will return this kind of Result.
pub type Result<T> = std::result::Result<T, Error>;

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
            debug!("received message: {:?}", msg); // TODO: trace
            self.handle_msg(msg).await?;
        }

        Ok(())
    }

    async fn handle_msg(&mut self, msg: RosSerialMsg) -> Result<()> {
        match msg.topic {
            Some(rosrust_msg::rosserial_msgs::TopicInfo::ID_TIME) => {
                self.handle_time_request().await?
            }
            Some(rosrust_msg::rosserial_msgs::TopicInfo::ID_LOG) => {
                self.handle_logging_request(msg).await?
            }
            Some(t) => warn!("unimplemented core topic: {:?}", t),
            _ => warn!("received unknown topic: {:?}", msg.topic),
        }

        Ok(())
    }

    async fn handle_time_request(&mut self) -> Result<()> {
        debug!("responding to time message");
        let time = rosrust::wall_time::now();
        let response = RosSerialMsg {
            topic: Some(rosrust_msg::rosserial_msgs::TopicInfo::ID_TIME),
            msg: time.encode_vec()?,
        };
        self.serial.send(response).await?;
        Ok(())
    }

    async fn handle_logging_request(&mut self, msg: RosSerialMsg) -> Result<()> {
        // TODO: use rosrust logging methods instead
        let log_msg = rosrust_msg::rosserial_msgs::Log::decode(&msg.msg[..])?;
        match log_msg.level {
            rosrust_msg::rosserial_msgs::Log::ROSDEBUG => {
                debug!("{}", log_msg.msg);
            }
            rosrust_msg::rosserial_msgs::Log::INFO => {
                info!("{}", log_msg.msg);
            }
            rosrust_msg::rosserial_msgs::Log::WARN => {
                warn!("{}", log_msg.msg);
            }
            rosrust_msg::rosserial_msgs::Log::ERROR => {
                error!("{}", log_msg.msg);
            }
            rosrust_msg::rosserial_msgs::Log::FATAL => {
                error!("{}", log_msg.msg);
            }
            _ => {
                error!("unimplemented log message level: {:?}", log_msg);
            }
        }
        Ok(())
    }

    async fn send_raw(&mut self, data: &[u8]) -> Result<()> {
        self.serial
            .send(RosSerialMsg {
                topic: None,
                msg: data.to_vec(),
            })
            .await?;
        Ok(())
    }

    async fn request_topics(&mut self) -> Result<()> {
        self.send_raw(b"\x00\x00\xff\x00\x00\xff").await
    }
}
