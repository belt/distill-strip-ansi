#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

mod parser;
mod strip;
mod stream;
#[cfg(feature = "std")]
mod writer;

#[cfg(feature = "filter")]
mod classifier;

pub use parser::{Action, Parser, State};
pub use stream::StripStream;
pub use strip::{contains_ansi, strip, strip_ansi_bytes, strip_ansi_escapes, strip_in_place, strip_into, strip_str};
#[cfg(feature = "std")]
pub use writer::StripWriter;

#[cfg(feature = "filter")]
mod filter;

#[cfg(feature = "filter")]
pub use classifier::{ClassifyingParser, SeqAction, SeqGroup, SeqKind};

#[cfg(feature = "filter")]
pub use filter::{filter_strip, filter_strip_into, filter_strip_str, FilterConfig, FilterMode, FilterStream};

#[cfg(feature = "toml-config")]
mod toml_config;

#[cfg(feature = "toml-config")]
pub use toml_config::{ConfigError, FilterToml, GeneralConfig, StripAnsiConfig};
