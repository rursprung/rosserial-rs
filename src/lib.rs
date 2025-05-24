//! Pure-Rust implementation of [rosserial](https://wiki.ros.org/rosserial).

#![forbid(unsafe_code)]
#![deny(warnings)]
#![deny(missing_docs)]
#![deny(missing_debug_implementations)]
#![deny(unused)]

mod codec;

use crate::codec::RosSerialMsgCodec;
use futures::SinkExt;
use log::{debug, error, info, trace, warn};
use rosrust::RosMsg;
use std::collections::HashMap;
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

/// Represents a ROS message as seen on ROS serial (i.e. as bytes).
#[derive(Debug, Default, PartialEq, Clone)]
pub struct RosSerialMsg {
    /// The topic on which the message has been sent / will be published. If [`None`] then it is a raw message and will be sent 1:1.
    pub topic: Option<u16>,
    /// The serialized message body.
    pub msg: Vec<u8>,
}

/// Represents a ROS Serial connection to a serial port.
//#[derive(Debug)]
#[allow(missing_debug_implementations)]
pub struct RosSerial<F> {
    serial: Framed<tokio_serial::SerialStream, RosSerialMsgCodec>,
    publisher_topics: HashMap<u16, rosrust_msg::rosserial_msgs::TopicInfo>,
    subscriber_topics: HashMap<u16, rosrust_msg::rosserial_msgs::TopicInfo>,
    pub_topics_handler_fn: F,
}

impl<F> RosSerial<F>
where
    F: AsyncFnMut(&str, &str, Vec<u8>) -> Result<()>
{
    /// Create a new ROS Serial connection.
    pub async fn new(serial: tokio_serial::SerialStream, pub_topics_handler_fn: F) -> Result<Self> {
        let serial = RosSerialMsgCodec.framed(serial);
        let mut this = RosSerial {
            serial,
            publisher_topics: HashMap::new(),
            subscriber_topics: HashMap::new(),
            pub_topics_handler_fn,
        };
        this.request_topics().await?;
        Ok(this)
    }

    /// Run the communication
    pub async fn run(&mut self) -> Result<()> {
        while let Some(Ok(msg)) = self.serial.next().await {
            trace!("received message: {:?}", msg);
            self.handle_msg(msg).await?;
        }

        Ok(())
    }

    async fn handle_msg(&mut self, msg: RosSerialMsg) -> Result<()> {
        const ID_SERVICE_SERVER_PUBLISHER: u16 = rosrust_msg::rosserial_msgs::TopicInfo::ID_SERVICE_SERVER + rosrust_msg::rosserial_msgs::TopicInfo::ID_PUBLISHER;
        const ID_SERVICE_SERVER_SUBSCRIBER: u16 = rosrust_msg::rosserial_msgs::TopicInfo::ID_SERVICE_SERVER + rosrust_msg::rosserial_msgs::TopicInfo::ID_SUBSCRIBER;
        const ID_SERVICE_CLIENT_PUBLISHER: u16 = rosrust_msg::rosserial_msgs::TopicInfo::ID_SERVICE_CLIENT + rosrust_msg::rosserial_msgs::TopicInfo::ID_PUBLISHER;
        const ID_SERVICE_CLIENT_SUBSCRIBER: u16 = rosrust_msg::rosserial_msgs::TopicInfo::ID_SERVICE_CLIENT + rosrust_msg::rosserial_msgs::TopicInfo::ID_SUBSCRIBER;
        match msg.topic {
            Some(rosrust_msg::rosserial_msgs::TopicInfo::ID_TIME) => {
                self.handle_time_request().await?
            }
            Some(rosrust_msg::rosserial_msgs::TopicInfo::ID_LOG) => {
                self.handle_logging_request(msg).await?
            }
            Some(rosrust_msg::rosserial_msgs::TopicInfo::ID_PUBLISHER) => {
                self.setup_publisher(msg).await?
            }
            Some(rosrust_msg::rosserial_msgs::TopicInfo::ID_SUBSCRIBER) => {
                self.setup_subscriber(msg).await?
            }
            Some(rosrust_msg::rosserial_msgs::TopicInfo::ID_PARAMETER_REQUEST) => {
                self.handle_parameter_request(msg).await?
            }
            Some(ID_SERVICE_SERVER_PUBLISHER) => warn!("unimplemented ID_SERVICE_SERVER_PUBLISHER! {:?}", msg),
            Some(ID_SERVICE_SERVER_SUBSCRIBER) => warn!("unimplemented ID_SERVICE_SERVER_SUBSCRIBER! {:?}", msg),
            Some(ID_SERVICE_CLIENT_PUBLISHER) => warn!("unimplemented ID_SERVICE_CLIENT_PUBLISHER! {:?}", msg),
            Some(ID_SERVICE_CLIENT_SUBSCRIBER) => warn!("unimplemented ID_SERVICE_CLIENT_SUBSCRIBER! {:?}", msg),
            Some(t) => {
                if let Some(topic_info) = self.publisher_topics.get(&t) {
                    (self.pub_topics_handler_fn)(topic_info.topic_name.as_str(), topic_info.message_type.as_str(), msg.msg).await?;
                } else {
                    warn!("unknown topic: {:?}", t);
                    self.request_topics().await?;
                }
            },
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

    async fn setup_publisher(&mut self, msg: RosSerialMsg) -> Result<()> {
        let topic_info = rosrust_msg::rosserial_msgs::TopicInfo::decode(&msg.msg[..])?;
        info!(
            "setting up publisher on {} [{}]",
            topic_info.topic_name, topic_info.message_type
        );
        self.publisher_topics
            .insert(topic_info.topic_id, topic_info);
        Ok(())
    }

    async fn setup_subscriber(&mut self, msg: RosSerialMsg) -> Result<()> {
        let topic_info = rosrust_msg::rosserial_msgs::TopicInfo::decode(&msg.msg[..])?;
        info!(
            "setting up subscriber on {} [{}]",
            topic_info.topic_name, topic_info.message_type
        );
        self.subscriber_topics
            .insert(topic_info.topic_id, topic_info);
        Ok(())
    }

    async fn handle_parameter_request(&mut self, msg: RosSerialMsg) -> Result<()> {
        let request = rosrust_msg::rosserial_msgs::RequestParamReq::decode(&msg.msg[..])?;
        debug!("handling parameter request: {:?}", request);
        // TODO: handle request
        let response = rosrust_msg::rosserial_msgs::RequestParamRes {
            floats: Vec::new(),
            ints: Vec::new(),
            strings: Vec::new(),
        };
        let response = RosSerialMsg {
            topic: Some(rosrust_msg::rosserial_msgs::TopicInfo::ID_PARAMETER_REQUEST),
            msg: response.encode_vec()?,
        };
        self.serial.send(response).await?;
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
