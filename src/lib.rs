//! # orchard

#![cfg_attr(docsrs, feature(doc_cfg))]
// Catch documentation errors caused by code changes.
#![deny(broken_intra_doc_links)]
#![deny(missing_debug_implementations)]
#![deny(missing_docs)]
#![deny(unsafe_code)]

mod address;
pub mod keys;

pub use address::Address;

/// Chain-specific constants and constraints for Orchard.
///
/// The purpose of this trait is to encapsulate things like the human-readable prefixes
/// for encoded addresses, or the range of allowable values for notes.
pub trait Chain {
}
