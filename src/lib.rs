#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

mod parser;
mod strip;
mod stream;

pub use parser::{Action, Parser, State};
pub use stream::StripStream;
pub use strip::{contains_ansi, strip, strip_in_place, strip_into, strip_str};
