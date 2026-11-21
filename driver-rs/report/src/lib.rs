//! The filter's logic that does not depend on WDF, so that plain `cargo test`
//! can test it (ADR 6).

#![forbid(unsafe_code)]

mod hid;
mod remap;
mod trim;

pub use hid::HidDescriptor;
pub use remap::{InputReport, Remap, USAGES, UnknownUsage, Usage, remap_target};
pub use trim::trim_descriptor;
