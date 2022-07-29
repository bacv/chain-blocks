use super::block::ParentHash;
use super::error::{Error, ErrorKind};
use super::Block;
use bytes::{Buf, BytesMut};
use futures::future::LocalBoxFuture;
use futures::prelude::*;
use pin_project::pin_project;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::io::Cursor;
use std::pin::Pin;
use std::rc::Rc;
use std::task::Context;
use std::task::Poll;
use tokio::io::AsyncRead;
use tokio_util::io::poll_read_buf;

const BUFFER_CAP: usize = 4096;

#[pin_project]
pub struct BlockStream<R: AsyncRead> {
    #[pin]
    reader: Option<R>,
    buf: BytesMut,
}

impl<R: AsyncRead> BlockStream<R> {
    pub fn new(reader: R) -> Self {
        BlockStream {
            reader: Some(reader),
            buf: BytesMut::with_capacity(BUFFER_CAP),
        }
    }
}

/// Returns a stream of Blocks.
/// All the errors returned as a result should be handled by the
/// caller and the stream should be terminated.
impl<R: AsyncRead> Stream for BlockStream<R> {
    type Item = super::Result<Block>;

    // Using ReaderStream code from tokio_util as reference to create a Block stream.
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.as_mut().project();

        let reader = match this.reader.as_pin_mut() {
            Some(r) => r,
            None => return Poll::Ready(None),
        };

        match poll_read_buf(reader, cx, &mut this.buf) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Err(err)) => {
                self.project().reader.set(None);
                Poll::Ready(Some(Err(Error::other_error(err.to_string()))))
            }
            Poll::Ready(Ok(0)) => {
                self.project().reader.set(None);
                Poll::Ready(None)
            }
            Poll::Ready(Ok(_)) => {
                let mut buf = Cursor::new(&this.buf[..]);

                // check method should be faster than parse method, so it's used here for speed and
                // lack of Block allocations that might happen while parsing.
                match Block::check(&mut buf) {
                    Ok(_) => {
                        let len = buf.position() as usize;
                        buf.set_position(0);

                        // If parsing fails with something other than Incomplete data error, then
                        // the stream should be terminated.
                        let maybe_block = Block::parse(&mut buf);
                        this.buf.advance(len);

                        Poll::Ready(Some(maybe_block))
                    }
                    Err(e) if *e.kind() == ErrorKind::Incomplete => Poll::Pending,
                    Err(e) => Poll::Ready(Some(Err(e))),
                }
            }
        }
    }
}

pub fn read_blocks<R: AsyncRead>(io: R) -> BlockStream<R> {
    BlockStream::new(io)
}

pub async fn find_common_ancestor<R>(
    blockchain_streams: &mut [BlockStream<R>],
) -> Result<Option<Block>, Error>
where
    R: tokio::io::AsyncRead + Unpin + Send,
{
    let stream_hashes: Rc<RefCell<HashMap<usize, HashSet<ParentHash>>>> = Rc::default();
    let blocks: Rc<RefCell<HashMap<ParentHash, Block>>> = Rc::default();
    let mut reads: Vec<LocalBoxFuture<()>> = vec![];

    // TODO: Parallelize stream reads and search for common ancestor either by using seperate thread
    // which receives stream id and hash via the channel or use some concurrent hashmap that is
    // being read in parallel.
    for (i, stream) in blockchain_streams.iter_mut().enumerate() {
        let stream_hashes_populate = stream_hashes.clone();
        let blocks = blocks.clone();
        // Multiple streams are parsing the blocks.
        let read = async move {
            tokio::pin!(stream);
            while let Some(res) = stream.next().await {
                match res {
                    // Every stream is populating the same hashmap.
                    Ok(block) => {
                        // Unique hash is expected here, consider stream invalid and end it.
                        if stream_hashes_populate.borrow().get(&i).is_some() {
                            return;
                        } else {
                            stream_hashes_populate
                                .borrow_mut()
                                .insert(i, HashSet::default());
                            blocks.borrow_mut().insert(block.parent_hash, block);
                        }
                    }
                    Err(_) => return,
                }
            }
        };
        reads.push(Box::pin(read));
    }

    // Running streams concurrently
    future::join_all(reads).await;

    // Filter the hashmap for common hashes.
    let mut common_hashes: HashSet<ParentHash> = HashSet::default();
    for (_, hashes) in stream_hashes.borrow_mut().iter() {
        common_hashes = common_hashes
            .intersection(hashes)
            .map(|x| x.to_owned())
            .collect();
    }

    // TODO: fix this to return the most recent common ancestor.
    if common_hashes.is_empty() {
        Ok(None)
    } else {
        let random_common_hash = common_hashes.iter().next().unwrap();
        let block = blocks.borrow().get(random_common_hash).unwrap().clone();
        Ok(Some(block))
    }
}

#[cfg(test)]
mod tests {
    use crate::stream::read_blocks;
    // TODO: This feels like a hack just to be able to use futures_mockstream::MockStream.
    // Change this to use a mocked stream that implements tokio::io::AsyncRead or add
    // compatibility directly to the BlockStreamm.
    use crate::utils::get_dummy_block_pair;
    use futures::StreamExt;
    use tokio_util::compat::FuturesAsyncReadCompatExt;

    #[tokio::test]
    async fn test_read_blocks_fn() {
        use futures_mockstream::MockStream;

        let (expected, reader) = get_dummy_block_pair();

        let ms = MockStream::from(&reader);
        let mut bs = read_blocks(ms.compat());
        while let Some(res) = bs.next().await {
            let block = res.unwrap();

            assert_eq!(expected.parent_hash, block.parent_hash);
            assert_eq!(expected.block_number, block.block_number);
            assert_eq!(expected.content, block.content);
        }
    }
}
