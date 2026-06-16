//! Wall-clock ecosystem bench for `vtparse`.
//!
//! `vtparse` is wezterm's low-level DEC ANSI state machine — the
//! crate `vte` (used by `strip-ansi-escapes`) is a sibling, not a
//! competitor in the strip-only sense. There's no "strip" function
//! shipped here; you implement `VTActor` and decide what to do with
//! each parsed event.
//!
//! For a like-for-like measurement we adopt the canonical "strip"
//! adapter: retain `print` chars (UTF-8 re-encoded) and `execute_c0_or_c1`
//! control bytes, drop everything else (CSI, OSC, DCS, ESC, APC, SOS,
//! PM). That's the same shape `strip-ansi-escapes` builds on top of
//! `vte`, so the comparison is fair.
//!
//! Caveat: vtparse decodes UTF-8 internally, so non-UTF-8 byte
//! sequences in the input are normalised to U+FFFD before being
//! emitted by `print`. The other crates here are byte-preserving.
//! For ANSI-shaped fixtures (cargo output, OSC 8 hyperlinks, the
//! generated dirty payload) the output bytes match; for arbitrary
//! binary input the answers diverge. Documenting it here keeps the
//! bench numbers honest.

use criterion::{Criterion, criterion_group, criterion_main};
use distill_bench_harness::{StripBench, run_strip_bench};
use vtparse::{CsiParam, VTActor, VTParser};

/// Minimal `VTActor` that turns the parsed event stream into a
/// stripped byte vector.
///
/// Allocation policy mirrors the other adapters: one `Vec<u8>` for
/// the output, sized to the input length on entry. UTF-8 encoding
/// of `print` chars uses a 4-byte stack scratch — no heap traffic
/// per character.
struct StripActor {
    out: Vec<u8>,
}

impl VTActor for StripActor {
    fn print(&mut self, b: char) {
        let mut buf = [0u8; 4];
        self.out.extend_from_slice(b.encode_utf8(&mut buf).as_bytes());
    }

    fn execute_c0_or_c1(&mut self, control: u8) {
        // Mirror strip-ansi-escapes / vte-based strippers: retain
        // C0/C1 control bytes (BEL, BS, HT, LF, CR, …) since they
        // shape printable output.
        self.out.push(control);
    }

    fn dcs_hook(&mut self, _: u8, _: &[i64], _: &[u8], _: bool) {}
    fn dcs_put(&mut self, _: u8) {}
    fn dcs_unhook(&mut self) {}
    fn esc_dispatch(&mut self, _: &[i64], _: &[u8], _: bool, _: u8) {}
    fn csi_dispatch(&mut self, _: &[CsiParam], _: bool, _: u8) {}
    fn osc_dispatch(&mut self, _: &[&[u8]]) {}
    fn apc_dispatch(&mut self, _: Vec<u8>) {}
}

fn strip_with_vtparse(input: &[u8]) -> Vec<u8> {
    let mut actor = StripActor {
        out: Vec::with_capacity(input.len()),
    };
    let mut parser = VTParser::new();
    parser.parse(input, &mut actor);
    actor.out
}

fn bench(c: &mut Criterion) {
    run_strip_bench(
        c,
        &StripBench {
            crate_name: "vtparse",
            bench_id: "vtparse",
            strip_fn: strip_with_vtparse,
        },
    );
}

criterion_group!(benches, bench);
criterion_main!(benches);
