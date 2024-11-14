//! # Tracing Tape
//! This crate contains the file format definition used by the
//! [tracing-tape-recorder](https://docs.rs/tracing-tape-recorder) and
//! [trace-deck](https://docs.rs/trace-deck) crates.
//! Have a look at the [getting stated
//! guide](https://github.com/soehrl/tracing-tape/wiki/Getting-Started) for more information.
//!
//! ## Versioning
//! The tape file format is subject to change in the future.
//! As such, the version is encoded in the tape file format itself as a major
//! and minor version. Parsing a tape file should be backwards and forwards
//! compatible across changes in the minor version.
//! E.g., a parser for version 1.2 should be able to parse version 1.3 and 1.1
//! files.
//!
//! The current tape file version is **`0.1`** which is also encoded in the
//! [VERSION](intro::Version) constant. The tape file format is versioned
//! independently of this crate.

pub mod intro;
pub mod record;
