#![crate_name = "chainbuf"]

#![deny(missing_docs)]
#![deny(warnings)]

#![feature(unsafe_destructor)]

//! The main crate for the Chainbuf library.
//!
//! ... docs are to be written
//!

// Stdlib dependencies
#[cfg(test)] extern crate test;

extern crate collections;

// Exetrnal dependencies
#[cfg(feature="nix")] extern crate nix;
#[cfg(feature="nix")] extern crate libc;

pub use chainbuf::Chain;

// XXX: for tests only, to remove, probably.
pub use chainbuf::CHB_MIN_SIZE;

// internal
mod chainbuf;
