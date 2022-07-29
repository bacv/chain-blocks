use bytes::{buf::Buf, Bytes};
use std::{fmt, io::Cursor};

use super::error::Error;

const PARENT_HASH_LEN: usize = 32;
const BLOCK_ID_LEN: usize = 8;
const PAYLOAD_SIZE_LEN: usize = 4;

pub type ParentHash = [u8; PARENT_HASH_LEN];

///
/// # Block wire protocol
///
/// In this example the block wire protocol is composed of a header and payload.
/// Parts of a header are not represented in a serialized block because they are only relevant
/// during the transport.
///
/// ## Structure
///
/// * First byte in a header represents the version of a protocol. Ideally first 4 bits could be
/// reserved for future uses and remaining 4 bits could be used for message type if the
/// communication flow would require it.
/// * Following 4 bytes indicates the lenght of the remaining data (payload).
/// * Next 32 bytes are for parent hash and other 8 bytes are for block number.
/// * Everything remaining are the contents of the block.
#[derive(Clone)]
pub struct Block {
    /// Block number, monotonically increasing as the chain grows.
    pub block_number: u64,
    /// Hash of the parent block.
    pub parent_hash: ParentHash,
    /// Block content.
    pub content: Box<[u8]>,
}

impl Block {
    pub fn new(parent_hash: ParentHash, block_number: u64, content: &[u8]) -> Block {
        Block {
            block_number,
            parent_hash,
            content: content.into(),
        }
    }

    pub fn check(src: &mut Cursor<&[u8]>) -> Result<(), Error> {
        match get_u8(src)? {
            0x1 => {
                let len = get_payload_len(src)?;

                // check if we have enough data for parsing.
                if src.get_ref().len() < len as usize {
                    return Err(Error::incomplete_error());
                }

                Ok(())
            }

            _ => Err(Error::incomplete_error()),
        }
    }

    pub fn parse(src: &mut Cursor<&[u8]>) -> Result<Block, Error> {
        match get_u8(src)? {
            // Version one, represented as 0b0000_0001 byte.
            // First most significant bits are reserved and last four bits are for the wire protocol
            // version.
            0x1 => {
                let len = get_payload_len(src)?;
                let len = len as usize;

                let parent_hash = get_parent_hash(src)?;
                let count = get_block_id(src)?;

                if src.remaining() < len {
                    return Err(Error::incomplete_error());
                }

                // As cursor moves we need to account for already read values.
                let len = len - PARENT_HASH_LEN - BLOCK_ID_LEN;
                let payload = Bytes::copy_from_slice(&src.chunk()[..len]);

                skip(src, len)?;

                Ok(Block::new(parent_hash, count, &payload[..]))
            }

            _ => Err(Error::incomplete_error()),
        }
    }
}

impl fmt::Display for Block {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {:?}", self.block_number, self.parent_hash)
    }
}

fn get_block_id(src: &mut Cursor<&[u8]>) -> Result<u64, Error> {
    let start = src.position() as usize;
    let end = start + BLOCK_ID_LEN - 1;
    if src.get_ref().len() > BLOCK_ID_LEN - 1 {
        // move the cursor after the hash data.
        src.set_position((end + 1) as u64);

        let mut buf = [0u8; 8];
        buf.copy_from_slice(&src.get_ref()[start..=end]);

        return Ok(u64::from_be_bytes(buf));
    }

    Err(Error::incomplete_error())
}

fn get_u8(src: &mut Cursor<&[u8]>) -> Result<u8, Error> {
    if !src.has_remaining() {
        return Err(Error::incomplete_error());
    }

    Ok(src.get_u8())
}

fn get_parent_hash(src: &mut Cursor<&[u8]>) -> Result<ParentHash, Error> {
    let start = src.position() as usize;
    let end = start + PARENT_HASH_LEN - 1;
    if src.get_ref().len() > PARENT_HASH_LEN - 1 {
        // move the cursor after the hash data.
        src.set_position((end + 1) as u64);

        let mut buf = [0u8; 32];
        buf.copy_from_slice(&src.get_ref()[start..=end]);

        return Ok(buf);
    }

    Err(Error::incomplete_error())
}

fn get_payload_len(src: &mut Cursor<&[u8]>) -> Result<u32, Error> {
    // check if we have 4 bytes available for readying.
    let start = src.position() as usize;
    let end = start + PAYLOAD_SIZE_LEN - 1;
    if src.get_ref().len() > PAYLOAD_SIZE_LEN - 1 {
        // move the cursor after the length data.
        src.set_position((end + 1) as u64);

        let mut buf = [0u8; 4];
        buf.copy_from_slice(&src.get_ref()[start..=end]);

        return Ok(u32::from_be_bytes(buf));
    }

    Err(Error::incomplete_error())
}

fn skip(src: &mut Cursor<&[u8]>, n: usize) -> Result<(), Error> {
    if src.remaining() < n {
        return Err(Error::incomplete_error());
    }

    src.advance(n);
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::utils::get_dummy_block_pair;

    use super::*;
    use bytes::{BufMut, BytesMut};
    use std::io::Cursor;

    #[test]
    fn test_check() {
        let (_, reader) = get_dummy_block_pair();

        let mut src = Cursor::new(&reader[..]);
        assert_eq!(Ok(()), Block::check(&mut src));
    }

    #[test]
    fn test_parsing() {
        let (expected, reader) = get_dummy_block_pair();

        let mut src = Cursor::new(&reader[..]);
        let block = Block::parse(&mut src).unwrap();

        assert_eq!(expected.parent_hash, block.parent_hash);
        assert_eq!(expected.block_number, block.block_number);
        assert_eq!(expected.content, block.content);
    }

    #[test]
    fn test_get_payload_len() {
        let cases = [
            (3, [0b0000_0000, 0b0000_0000, 0b0000_0000, 0b0000_0011]),
            (
                16777217,
                [0b0000_0001, 0b0000_0000, 0b0000_0000, 0b0000_0001],
            ),
        ];

        let mut buf = BytesMut::zeroed(4);
        for (expected, len_in_bytes) in cases {
            buf.copy_from_slice(&len_in_bytes);
            let mut buf = Cursor::new(&buf[..]);
            assert_eq!(expected, get_payload_len(&mut buf).unwrap());
        }
    }

    #[test]
    fn test_get_parent_hash() {
        // This is dummy suffix for parent hash tests, it imitates the subsequent bytes that follow
        // after the parent_hash.
        let dummy_suffix = [0x01, 0x0, 0x0, 0x0, 0x0];
        let cases = [(
            b"this is 32 character as bytes 32",
            b"this is 32 character as bytes 32",
        )];

        // Five dummy and 32 parent hash bytes.
        let mut buf = Vec::with_capacity(37);
        for (expected, parent_hash) in cases {
            buf.put(&parent_hash[..]);
            buf.put(&dummy_suffix[..]);

            let mut buf = Cursor::new(&buf[..]);
            assert_eq!(expected, &get_parent_hash(&mut buf).unwrap());
        }
    }
}
