#!/bin/sh
# Run the benchmark generator with captured, separated logs.
#
# Usage: bin/bench-run.sh <log-name> [generator args...]
#
# Why a wrapper rather than redirection baked into each mise task:
# every bench task wants the same four things (separate stdout/stderr
# capture, wall-clock total, a warning count, and the generator's real
# exit status), and duplicating that across `bench`, `bench:quick`,
# `bench:callgrind` and `bench:publish` guarantees they drift.
#
# Stream separation is deliberate. Criterion writes progress and
# warnings to stderr and statistical results to stdout; merging them
# into one file is analysable but merging them through one *pipe* also
# risks reordering, because stdout is block-buffered when not a TTY
# while stderr is not. Keeping them apart preserves both files as
# clean inputs for later analysis.
#
# Logs land under target/ (already gitignored) with a per-task name, so
# a quick iteration run cannot clobber the log from a publication run.
set -eu

if [ "$#" -lt 1 ]; then
    echo "usage: $0 <log-name> [generator args...]" >&2
    exit 2
fi

name=$1
shift

log_dir=target/bench-logs
mkdir -p "$log_dir"
out="$log_dir/$name.out"
err="$log_dir/$name.err"
rc_file="$log_dir/.$name.rc"

start=$(date +%s)

# The generator's exit status has to survive the pipe to `tee`. POSIX
# sh has no pipefail, so stash it in a file inside the subshell.
{ ./bin/generate-benchmarks-md.py "$@" 2>"$err"; echo $? >"$rc_file"; } | tee "$out"

rc=$(cat "$rc_file")
rm -f "$rc_file"
end=$(date +%s)

# Criterion's sample-size warnings go to stderr, which is captured
# rather than displayed — so surface a definitive count here instead of
# leaving the operator to spot them in a 45-minute scrollback.
warnings=$(grep -c 'Unable to complete' "$err" 2>/dev/null || true)
[ -n "$warnings" ] || warnings=0

echo "" >&2
echo "bench: elapsed $((end - start))s" >&2
echo "bench: stdout  $out" >&2
echo "bench: stderr  $err" >&2
echo "bench: criterion sample-size warnings: $warnings" >&2

exit "$rc"
