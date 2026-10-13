"""Builds must run on the reference machine's 2011 CPU (ADR 0013, D5.8).

The i7-2600 (Sandy Bridge) has AVX but not AVX2: a build tuned for a newer
CPU dies there with "illegal instruction". The baseline is x86-64-v2 at most,
so no build, packaging or CI file may ask for more.
"""

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

# Settings that make the compiler assume a newer CPU than the baseline.
FORBIDDEN = re.compile(
    r"target-cpu|x86-64-v[34]|-m(?:arch|tune)=|[+-]?\bavx2\b|\bavx512|\+fma\b|-mfma",
    re.IGNORECASE,
)

SUFFIXES = {".toml", ".yml", ".yaml", ".cfg", ".spec", ".sh", ".ps1", ".iss", ".json", ""}


def build_files() -> list[Path]:
    files = [ROOT / "Cargo.toml", ROOT / "pyproject.toml", ROOT / "rust-toolchain.toml"]
    files += [p for p in (ROOT / ".cargo").glob("config*") if p.is_file()]
    files += (ROOT / "crates").glob("*/Cargo.toml")
    files += (ROOT / ".github" / "workflows").glob("*.y*ml")
    files += [
        p
        for p in (ROOT / "packaging").rglob("*")
        if p.is_file() and p.suffix in SUFFIXES and p.stat().st_size < 1_000_000
    ]
    return [p for p in files if p.is_file()]


def test_no_build_targets_a_newer_cpu_than_the_baseline() -> None:
    files = build_files()
    assert len(files) > 5, "the scan found the build files"
    offending = []
    for path in files:
        text = path.read_text(encoding="utf-8", errors="replace")
        for number, line in enumerate(text.splitlines(), start=1):
            if FORBIDDEN.search(line):
                offending.append(f"{path.relative_to(ROOT)}:{number}: {line.strip()}")
    assert not offending, "builds must stay at x86-64-v2 or below:\n" + "\n".join(offending)


def test_the_pattern_catches_what_it_must() -> None:
    for bad in (
        'rustflags = ["-C", "target-cpu=native"]',
        "RUSTFLAGS: -Ctarget-feature=+avx2",
        "CFLAGS=-march=haswell",
        "target = x86-64-v3",
    ):
        assert FORBIDDEN.search(bad), bad
    for fine in ("x86_64-unknown-linux-gnu", "x86-64-v2", "universal2", "avx", "manylinux"):
        assert not FORBIDDEN.search(fine), fine
