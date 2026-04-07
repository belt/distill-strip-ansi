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
pub use strip::{contains_ansi, contains_ansi_c1, strip, strip_ansi_bytes, strip_ansi_escapes, strip_in_place, strip_into, strip_str, try_strip_str};
#[cfg(feature = "std")]
pub use writer::StripWriter;

#[cfg(feature = "filter")]
mod filter;

#[cfg(feature = "filter")]
pub use classifier::{ClassifyingParser, SeqAction, SeqDetail, SeqGroup, SeqKind, SgrContent, OscType, map_osc_number};

#[cfg(feature = "filter")]
pub use filter::{filter_strip, filter_strip_into, filter_strip_str, try_filter_strip_str, FilterConfig, FilterMode, FilterStream};

#[cfg(feature = "filter")]
mod preset;

#[cfg(feature = "filter")]
pub use preset::TerminalPreset;

#[cfg(feature = "terminal-detect")]
mod detect;

#[cfg(feature = "terminal-detect")]
pub use detect::{detect_preset, detect_preset_untrusted, detect_sgr_mask, detect_sgr_mask_untrusted};

#[cfg(feature = "toml-config")]
mod toml_config;

#[cfg(feature = "toml-config")]
pub use toml_config::{ConfigError, FilterToml, GeneralConfig, StripAnsiConfig};

#[cfg(feature = "toml-config")]
mod threat_db;

#[cfg(feature = "toml-config")]
pub use threat_db::{ThreatDb, ThreatEntry};

#[cfg(feature = "downgrade-color")]
pub mod downgrade;

#[cfg(feature = "transform")]
pub mod sgr_rewrite;

#[cfg(feature = "transform")]
mod transform_stream;

#[cfg(feature = "transform")]
pub use transform_stream::{TransformConfig, TransformSlice, TransformSlices, TransformStream};

#[cfg(feature = "color-palette")]
pub mod palette;
