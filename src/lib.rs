#![crate_name = "chainbuf"]
#![comment = "Fast chained buffers"]
#![license = "MIT"]

#![deny(missing_doc)]
#![deny(warnings)]


//! The main crate for the Chainbuf library.
//!
//! ... docs are to be written
//!

// Stdlib dependencies
#[cfg(test)] extern crate test;

extern crate collections;



pub use chainbuf::Chain;
pub use chainbuf::CHB_MIN_SIZE;

// internal
mod chainbuf;
