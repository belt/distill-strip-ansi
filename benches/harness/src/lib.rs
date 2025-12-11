//! Shared benchmark harness for distill-strip-ansi.
//!
//! Provides consistent measurement config, hardware-adaptive input
//! sizes, resource capture (RSS/CPU), and JSON flush — so every
//! bench binary in the workspace measures apples-to-apples.


mod cache;
mod config;
mod inputs;
mod resources;
mod runner;

pub use cache::CacheInfo;
pub use config::BenchConfig;
pub use inputs::{
    InputMeta, InputSource, clean_input, dirty_input, fmt_bytes, load_fixture,
    select_input,
};
pub use resources::{CapturePoint, FlushParams, ResourceTracker, flush_resources};
pub use runner::{StripBench, run_strip_bench};

/// Re-export resources module for direct access to helpers.
pub mod resources_pub {
    pub use crate::resources::current_rss_bytes_pub;
}
