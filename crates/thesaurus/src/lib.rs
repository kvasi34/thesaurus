//! Redis-compatible, async in-memory key-value store.
//!
//! Thesaurus implements a subset of the Redis command set over the RESP2 protocol.
//! The core storage is a shared [`store::Store`] accessed concurrently by async
//! [`handler::Handler`] tasks, each of which drives one TCP client connection.
//! Optional AOF persistence is provided by [`aof`].

pub mod aof;
pub mod command;
pub mod config;
pub mod errors;
pub mod executor;
pub mod handler;
pub mod resp2;
pub mod store;
pub mod ttl;
