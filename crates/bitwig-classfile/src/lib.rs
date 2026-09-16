//! Class-file and JAR surgery primitives.
//!
//! Three layers, cheapest first:
//!
//! - [`pool`] reads and rewrites constant pool strings without parsing a class.
//! - [`edit`] parses, edits and reassembles a class through krakatau2.
//! - [`jar`] reads archive entries and writes a rewritten archive.
//!
//! Nothing here knows anything about Bitwig.

pub mod edit;
pub mod jar;
pub mod pool;

pub use jar::{Jar, JarEdits};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io error on {path}: {source}")]
    Io { path: String, source: std::io::Error },
    #[error("archive error: {0}")]
    Archive(String),
    #[error("no entry named {0}")]
    EntryNotFound(String),
    #[error("not a class file")]
    NotAClass,
    #[error("constant pool truncated at offset {at}")]
    TruncatedPool { at: usize },
    #[error("unknown constant pool tag {tag} at offset {at}")]
    UnknownPoolTag { at: usize, tag: u8 },
    #[error("string of {0} bytes does not fit a constant pool entry")]
    StringTooLong(usize),
    #[error("method not found")]
    MethodNotFound,
    #[error("class parse failed: {0}")]
    ClassParse(String),
    #[error("assembly failed: {0}")]
    Assembly(String),
}

pub type Result<T> = std::result::Result<T, Error>;
