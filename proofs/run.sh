#!/usr/bin/env bash
# Check every Verus-verified kernel in this repository.
#
# Usage: proofs/run.sh [path-to-verus]
#
# The Verus binary defaults to $VERUS, then to `verus` on PATH. Verus finds
# Z3 through VERUS_Z3_PATH or PATH. See proofs/README.md for the pinned
# Verus revision and how to build it.
set -euo pipefail

verus_bin="${1:-${VERUS:-verus}}"
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

exec "$verus_bin" \
    --crate-type=lib \
    --triggers-mode silent \
    "$repo_root/proofs/verus_root.rs"
