
# Chainbuf
[![Build Status](https://travis-ci.org/little-arhat/rust-chainbuf.svg?branch=master)](https://travis-ci.org/little-arhat/rust-chainbuf)

Chained buffer of contigious byte chunks.

# Simple usage

Plug the package into your app via Cargo:

```toml
[dependencies]
chainbuf = "0.0.4"
```

then use it:

```rust
extern crate chainbuf;
use chainbuf::Chain;
let mut chain = Chain::new();
chain.append_bytes("helloworld".as_bytes());
let some_bytes = chain.pullup(2);
```

# Details of implementation
Chainbuf consists of linked list of nodes, with `start` and `end`
offsets and a reference counted pointer to DataHolder. DataHolders can be
shared across different chains, so for mutation new nodes and data holders
are created (as in Copy-On-Write).
