use Error::*;
use std::fmt::{Display, Formatter};
use tokio_util::bytes::{Buf, BufMut, BytesMut};
use tokio_util::codec::{Decoder, Encoder};

/// All methods in this module will return this kind of Result.
#[derive(Debug)]
pub enum Error {
    /// The message did not use a protocol version supported by the current implementation.
    WrongProtocolVersion(u8),
    /// The checksum validation for the message length failed.
    InvalidLengthChecksum(u8),
    /// The checksum validation for the message body failed.
    InvalidMessageChecksum(u8),
    /// An underlying IO error occurred.
    IoError(std::io::Error),
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        IoError(value)
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            WrongProtocolVersion(v) => write!(
                f,
                "wrong protocol version: {} (expected: {})",
                v, PROTOCOL_VERSION_2
            ),
            InvalidLengthChecksum(checksum) => write!(
                f,
                "invalid length header checksum: computed {} but expected 255",
                checksum
            ),
            InvalidMessageChecksum(checksum) => write!(
                f,
                "invalid message checksum: computed {} but expected 255",
                checksum
            ),
            IoError(_) => write!(f, "IO error"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            IoError(e) => Some(e),
            _ => None,
        }
    }
}

/// All methods in this module will return this kind of Result.
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Default, PartialEq, Clone)]
pub struct RosSerialMsg {
    pub(crate) topic: Option<u16>,
    pub(crate) msg: Vec<u8>,
}

const HEADER: u8 = b'\xff';
const PROTOCOL_VERSION_2: u8 = b'\xfe';

pub struct RosSerialMsgCodec;

fn calc_checksum(bytes: impl Iterator<Item = u8>) -> u8 {
    let mut checksum: u16 = 0;
    for byte in bytes {
        checksum += byte as u16;
        checksum = checksum % 256;
    }
    checksum as u8
}

impl Decoder for RosSerialMsgCodec {
    type Item = RosSerialMsg;
    type Error = Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>> {
        let start_marker = src.as_ref().iter().position(|b| *b == b'\xff');
        if let Some(n) = start_marker {
            // Throw away trash before the marker - we are not interested in it. Also skip the marker itself
            src.advance(n + 1);
        } else {
            return Ok(None);
        }

        match src.try_get_u8() {
            Ok(PROTOCOL_VERSION_2) => {}
            Ok(version) => return Err(WrongProtocolVersion(version)),
            Err(_) => return Ok(None),
        }

        let len = match src.try_get_i16_le() {
            Ok(len) => len,
            Err(_) => return Ok(None),
        };
        let checksum_filler = match src.try_get_u8() {
            Ok(checksum_filler) => checksum_filler,
            Err(_) => return Ok(None),
        };
        // validate len checksum
        let checksum = calc_checksum(
            len.to_le_bytes()
                .iter()
                .chain(checksum_filler.to_le_bytes().iter())
                .copied(),
        );
        if checksum != 255 {
            return Err(InvalidLengthChecksum(checksum));
        }

        let topic_id = match src.try_get_u16_le() {
            Ok(topic_id) => topic_id,
            Err(_) => return Ok(None),
        };

        if src.len() < len as usize {
            return Ok(None);
        }
        let data = src.split_to(len as usize).to_vec();

        // data checksum
        let data_checksum = loop {
            let data_checksum = match src.try_get_u8() {
                Ok(data_checksum) => data_checksum,
                Err(_) => return Ok(None),
            };
            if data_checksum != 0 {
                break data_checksum;
            }
        };

        let checksum = calc_checksum(
            topic_id
                .to_le_bytes()
                .iter()
                .chain(data.iter())
                .chain(data_checksum.to_le_bytes().iter())
                .copied(),
        );

        if checksum != 255 {
            return Err(InvalidMessageChecksum(checksum as u8));
        }

        Ok(Some(RosSerialMsg {
            topic: Some(topic_id),
            msg: data,
        }))
    }
}

impl Encoder<RosSerialMsg> for RosSerialMsgCodec {
    type Error = Error;

    fn encode(&mut self, item: RosSerialMsg, dst: &mut BytesMut) -> Result<()> {
        dst.put_u8(HEADER);
        dst.put_u8(PROTOCOL_VERSION_2);

        // hack(ish) way of sending raw messages
        if item.topic.is_none() {
            dst.put_slice(item.msg.as_slice());
            return Ok(());
        }

        let length_bytes = (item.msg.len() as u16).to_le_bytes();
        dst.put_slice(length_bytes.as_slice());

        let length_checksum = calc_checksum(length_bytes.iter().copied());
        dst.put_u8(length_checksum);

        let topic_bytes = item.topic.unwrap().to_le_bytes();
        dst.put_slice(topic_bytes.as_slice());

        dst.put_slice(item.msg.as_slice());

        let msg_checksum = 255 - calc_checksum(topic_bytes.iter().chain(item.msg.iter()).copied());
        dst.put_u8(msg_checksum);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use tokio_stream::StreamExt;

    #[tokio::test]
    async fn test_request_topics() {
        env_logger::init();
        let stream = Cursor::new(b"\xff\xfe\x08\0\xf7\n\0\0\0\0\0\0\0\0\0\xf5".to_vec());
        let mut codec = RosSerialMsgCodec.framed(stream);

        if let Some(Ok(msg)) = codec.next().await {
            assert_eq!(msg.topic, Some(10));
            let count_nonzero = msg.msg.iter().filter(|&b| *b != b'\0').count();
            assert_eq!(count_nonzero, 0);
        }
        assert!(codec.next().await.is_none());
    }
}
