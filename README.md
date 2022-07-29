# Chain Blocks
Async implementation of chained block streaming and parsing

## To people who read this
Hi :) I've spent too much time on making sure that the parsing is working correctly so I didn't manage to make common ancestor search parallelized. Also tests for streaming and search are missing at the moment, but I'm planning to update this repo in short time.

## Block wire protocol

In this example the block wire protocol is composed of a header and payload. Parts of a header are not represented in a serialized block because they are only relevant during the transport.

### Structure

* First byte in a header represents the version of a protocol. Ideally first 4 bits could be reserved for future uses and remaining 4 bits could be used for message type if the communication flow would require it.
* Following 4 bytes indicates the lenght of the remaining data (payload).
* Next 32 bytes are for parent hash and other 8 bytes are for block number.
* Everything remaining are the contents of the block.

