#![no_std]
#![warn(missing_docs)]
// Here rather than in Cargo.toml's [lints], which Cargo before 1.74 ignores.
#![warn(clippy::all, clippy::pedantic)]
#![doc = include_str!("../README.md")]

#[cfg(not(test))]
mod panic;
