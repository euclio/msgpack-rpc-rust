//! This crate provides facilities to use the MessagePack remote procedure call system
//! (msgpack-RPC) in Rust.

#![warn(missing_docs)]

extern crate mioco;
extern crate rmp as msgpack;
extern crate rmp_serde;

mod client;
mod message;
mod server;

pub use client::*;
pub use server::*;
