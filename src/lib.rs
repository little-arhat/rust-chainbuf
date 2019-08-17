#![crate_name = "chainbuf"]
#![deny(missing_docs)]
#![deny(warnings)]

//! The main crate for the Chainbuf library.
//!
//! ... docs are to be written
//!

pub use chainbuf::Chain;

// XXX: for tests only, to remove, probably.
pub use chainbuf::CHB_MIN_SIZE;

// internal
mod chainbuf;
