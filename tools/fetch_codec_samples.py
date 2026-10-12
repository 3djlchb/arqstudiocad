"""Fetch opencadcodec's sample drawings for the round-trip corpus.

Downloads the codec's own DWG/DXF test files (MPL-2.0), at the commit pinned
in Cargo.toml, into ``target/corpus/opencadcodec/`` (ignored by git) and
checks each one against its SHA-256. Run it after changing the pinned
commit, updating the list below. See ADR 0011, D3.12.

Usage: ``uv run python tools/fetch_codec_samples.py``
"""

from __future__ import annotations

import hashlib
import sys
import urllib.request
from pathlib import Path

#: Commit of opencadcodec pinned in Cargo.toml (rev 8189f83).
COMMIT = "8189f833574c544fc57a1d773591014c3a5907c9"
BASE_URL = f"https://raw.githubusercontent.com/HakanSeven12/opencadcodec/{COMMIT}/"

#: Path in the codec repository → SHA-256 of the file at COMMIT, as GitHub
#: serves it (LF line endings; a Windows git checkout may differ).
SAMPLES: dict[str, str] = {
    "tests/anonymous-names.dwg": "ea5b55f7e99d2ad412779ef7f3e71ff3bd7f6c4147f6217d936eed02171291b9",
    "tests/datatable/point_object_id.dwg": (
        "9eef9375c77d72881dc202b9fb94b9c414a048dfdfab8374419b44830337c49d"
    ),
    "tests/datatable/point_object_id.dxf": (
        "6e45969038d3f5777ac6b8fe52e7409ccd7f776409f528a4ff5781c9af7b431f"
    ),
    "tests/fixtures/dynamic-blocks/lookup.dwg": (
        "290a8696fff1328f29769dda2c5106fdc9b1f7373d010790271606dbf008df69"
    ),
    "tests/fixtures/dynamic-blocks/point.dwg": (
        "808f0a0d7ad029c6a23fed4fd87323fd03d6938f4feee2d8ab64c2e70e21f232"
    ),
    "tests/fixtures/dynamic-blocks/polar.dwg": (
        "8bdea037b2a59e33be11c5b0267530f0c48123746981007f738affc8d34b67b8"
    ),
    "tests/fixtures/dynamic-blocks/xy.dwg": (
        "044844db69865b22dbe8a9407e3d94abe15af416ad9a03f56765c3e28d7df983"
    ),
    "tests/fixtures/viewports/reference-angle-units.dxf": (
        "2f042e4fd81023545752647563e4f2b84cf185ec58c51681e5e3ee9aaa4189b8"
    ),
    "tests/fixtures/viewports/reference-clip-activation.dxf": (
        "ae3fed1aa164bb6d1d06ec5765b0b7f31c33fd066628dabf75441fd0ec03ab3f"
    ),
    "tests/issue68/r12_mleaderstyle_repro.dxf": (
        "d5c1afbb6df760e67b7ea22799ad64457e0409f48ac5e9c5f10a741c81778fbf"
    ),
    "tests/mpolygon-fixture.dwg": (
        "13a9a2b46740174f41467603f5baf3b8186c5b6726ab1e43e529ef7c1a2a5cdd"
    ),
}

DESTINATION = Path(__file__).resolve().parent.parent / "target" / "corpus" / "opencadcodec"


def sha256(data: bytes) -> str:
    """Hex SHA-256 of ``data``."""
    return hashlib.sha256(data).hexdigest()


def fetch(path: str, expected: str) -> str:
    """Make sure ``path`` is present and intact; returns what was done."""
    target = DESTINATION / path.removeprefix("tests/")
    if target.is_file() and sha256(target.read_bytes()) == expected:
        return "present"
    with urllib.request.urlopen(BASE_URL + path, timeout=60) as response:
        data: bytes = response.read()
    if sha256(data) != expected:
        raise RuntimeError(f"{path}: checksum mismatch, refusing to keep it")
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_bytes(data)
    return "downloaded"


def main() -> int:
    """Fetch every sample; non-zero exit if any failed."""
    failed = 0
    for path, expected in SAMPLES.items():
        try:
            print(f"{fetch(path, expected):>10}  {path}")
        except (OSError, RuntimeError) as error:
            failed += 1
            print(f"{'FAILED':>10}  {path}: {error}", file=sys.stderr)
    print(f"{len(SAMPLES) - failed}/{len(SAMPLES)} samples in {DESTINATION}")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
