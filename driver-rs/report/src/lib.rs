//! The filter's logic that does not depend on WDF, so that plain `cargo test`
//! can test it (ADR 6).

#![forbid(unsafe_code)]

mod arbiter;
mod bindings;
mod device;
mod event;
mod gesture;
mod hid;
mod output;
mod reads;
mod recognizer;
mod time;
mod trim;

pub use bindings::{Bindings, Config, LineError};
pub use device::{InputReport, KeyReport, Usage};
pub use hid::HidDescriptor;
pub use reads::{Action, ReadState, SerialCompleter, Token};
pub use recognizer::Recognizer;
pub use time::Time;
