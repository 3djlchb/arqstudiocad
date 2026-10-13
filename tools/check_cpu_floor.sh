#!/usr/bin/env bash
# Runs the test suites on an emulated Intel Sandy Bridge CPU (ADR 0013, D5.8).
#
# The floor machine's i7-2600 has AVX but not AVX2. Intel's Software
# Development Emulator in Sandy Bridge mode (`sde64 -snb`) stops a program at
# the first instruction that CPU lacks, so every Rust test binary and the
# Python suite (Qt, PySide6, NumPy included) run here exactly as far as that
# CPU allows. (QEMU cannot do this: with `-cpu SandyBridge` it hides AVX2 from
# CPUID but still executes it.)
#
# Intel SDE is free but proprietary: download it yourself from
# https://www.intel.com/content/www/us/en/developer/articles/tool/software-development-emulator.html
# (Linux package), extract it, and point SDE at its sde64, or put it in
# ~/tools/sde*/. It is never part of the repository or the application.
#
# Linux or WSL, x86-64. Usage, from the repository root:
#   bash tools/check_cpu_floor.sh            # or SDE=/path/to/sde64 bash ...
set -euo pipefail

CHIP=snb  # Sandy Bridge
SDE=${SDE:-$(command -v sde64 || ls -d "$HOME"/tools/sde*/sde64 2>/dev/null | tail -1 || true)}
if [[ -z "$SDE" || ! -x "$SDE" ]]; then
    echo "Intel SDE (sde64) not found: set SDE=/path/to/sde64 or extract it in ~/tools/" >&2
    exit 2
fi
if [[ "$(uname -m)" != "x86_64" ]]; then
    echo "this check emulates an x86-64 CPU on an x86-64 host" >&2
    exit 2
fi
RUNNER="$SDE -$CHIP --"
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

echo "== 1/3 The emulator really rejects AVX2 (else this check proves nothing)"
cat > "$WORK/avx2.rs" <<'EOF'
use std::arch::x86_64::*;
#[target_feature(enable = "avx2")]
unsafe fn add() -> i32 {
    let a = _mm256_set1_epi32(1);
    _mm256_extract_epi32::<0>(_mm256_add_epi32(a, a))
}
fn main() {
    std::process::exit(unsafe { add() });
}
EOF
rustc -O -o "$WORK/avx2" "$WORK/avx2.rs"
"$WORK/avx2" && native=$? || native=$?
if [[ $native -ne 2 ]]; then
    echo "the host itself cannot run AVX2 (exit $native): nothing to compare" >&2
    exit 2
fi
$RUNNER "$WORK/avx2" > "$WORK/avx2.log" 2>&1 && emulated=$? || emulated=$?
if [[ $emulated -eq 2 ]]; then
    echo "FAIL: $RUNNER ran an AVX2 instruction; it does not limit the CPU" >&2
    exit 1
fi
echo "ok: AVX2 runs natively (exit 2) and is refused as Sandy Bridge (exit $emulated)"

echo "== 2/3 Rust tests as Sandy Bridge"
CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUNNER="$RUNNER" cargo test --workspace --exclude arq-python

echo "== 3/3 Python tests as Sandy Bridge (Qt offscreen)"
uv sync --quiet
PYTHON=$(uv run python -c 'import sys; print(sys.executable)')
QT_QPA_PLATFORM=offscreen $RUNNER "$PYTHON" -m pytest -q -p no:cacheprovider

echo "== The test suites pass on an emulated Sandy Bridge CPU."
