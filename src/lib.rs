#![crate_name = "chainbuf"]
#![comment = "Fast chained buffers"]
#![license = "MIT"]

#![deny(missing_doc)]
#![deny(warnings)]

#![feature(if_let)]

//! The main crate for the Chainbuf library.
//!
//! ... docs are to be written
//!

// Stdlib dependencies
#[cfg(test)] extern crate test;

extern crate collections;

// Exetrnal dependencies
#[cfg(feature="nix")] extern crate nix;

pub use chainbuf::Chain;

// XXX: for tests only, to remove, probably.
pub use chainbuf::CHB_MIN_SIZE;

// internal
mod chainbuf;
