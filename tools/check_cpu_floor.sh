#!/usr/bin/env bash
# Runs the test suites on an emulated Intel Sandy Bridge CPU (ADR 0013, D5.8).
#
# The reference machine's i7-2600 has AVX but not AVX2. QEMU's user-mode
# emulator with `-cpu SandyBridge` raises "illegal instruction" for anything
# newer, so every Rust test binary and the Python suite (Qt, PySide6, NumPy
# included) run here exactly as far as that CPU allows.
#
# Linux or WSL only. Needs `qemu-user` (sudo apt install qemu-user), cargo and
# uv. Usage, from the repository root:  bash tools/check_cpu_floor.sh
set -euo pipefail

CPU=SandyBridge
QEMU=$(command -v qemu-x86_64 || command -v qemu-x86_64-static || true)
if [[ -z "$QEMU" ]]; then
    echo "qemu-x86_64 not found: sudo apt install qemu-user" >&2
    exit 2
fi
if [[ "$(uname -m)" != "x86_64" ]]; then
    echo "this check emulates an x86-64 CPU on an x86-64 host" >&2
    exit 2
fi
RUNNER="$QEMU -cpu $CPU"
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

echo "== 1/3 The emulation really rejects AVX2 (else this check proves nothing)"
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
$RUNNER "$WORK/avx2" && emulated=$? || emulated=$?
if [[ $emulated -eq 2 ]]; then
    echo "FAIL: $RUNNER ran an AVX2 instruction; the emulation does not limit the CPU" >&2
    exit 1
fi
echo "ok: AVX2 runs natively (exit 2) and is refused under $CPU (exit $emulated)"

echo "== 2/3 Rust tests on $CPU"
CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUNNER="$RUNNER" cargo test --workspace --exclude arq-python

echo "== 3/3 Python tests on $CPU (Qt offscreen)"
uv sync --quiet
PYTHON=$(uv run python -c 'import sys; print(sys.executable)')
QT_QPA_PLATFORM=offscreen $RUNNER "$PYTHON" -m pytest -q -p no:cacheprovider

echo "== The test suites pass on an emulated $CPU CPU."
