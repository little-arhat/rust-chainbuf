#![crate_name = "chainbuf"]

#![deny(missing_docs)]
#![deny(warnings)]

#![feature(unsafe_destructor)]

//! The main crate for the Chainbuf library.
//!
//! ... docs are to be written
//!

// Stdlib dependencies
#[allow(unstable)] #[cfg(test)] extern crate test;

#[allow(unstable)] extern crate collections;

// Exetrnal dependencies
#[cfg(feature="nix")] extern crate nix;
#[allow(unstable)] #[cfg(feature="nix")] extern crate libc;

pub use chainbuf::Chain;

// XXX: for tests only, to remove, probably.
pub use chainbuf::CHB_MIN_SIZE;

// internal
mod chainbuf;
