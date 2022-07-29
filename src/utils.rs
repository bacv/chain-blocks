use bytes::{BufMut, Bytes};

use super::block::Block;

pub fn get_dummy_block_pair() -> (Block, Bytes) {
    let parent_hash = b"this is 32 character as 32 bytes";
    let count = 42u64;
    let payload = b"utxo_utxo_utxo";

    let expected_block = Block::new(*parent_hash, count, payload);

    let mut buf = Vec::with_capacity(2000);
    buf.put(&parent_hash[..]);
    buf.put(&count.to_be_bytes()[..]);
    buf.put(&payload[..]);

    let size = u32::to_be_bytes(buf.len() as u32);
    let mut reader = Vec::with_capacity(2000);
    reader.put(&b"\x01"[..]);
    reader.put(&size[..]);
    reader.put(&buf[..]);
    // Adding some garbage at the end of the buffer to imitate a longer stream.
    reader.put(&buf[..]);

    (expected_block, reader.into())
}
