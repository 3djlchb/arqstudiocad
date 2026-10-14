"""Fetch the DWG/DXF engine's sample drawings for the round-trip corpus.

Downloads the AcadRust package (MPL-2.0) of the version pinned in
Cargo.toml from crates.io, checks it against the checksum Cargo recorded in
Cargo.lock, and extracts its test drawings into
``target/corpus/acadrust/`` (ignored by git), each checked against its
SHA-256. Run it after changing the pinned version, updating the list below.
See ADR 0011, D3.12, and ADR 0017.

Also downloads regression drawings from LibreDWG's test data (GPL-3.0,
used as test input only, never shipped): the same constraint drawing in
seven DWG versions, whose old-layout objects the engine keeps undecoded
(ADR 0017 D2, D5.4), into ``target/corpus/libredwg/``.

Usage: ``uv run python tools/fetch_codec_samples.py``
"""

from __future__ import annotations

import hashlib
import io
import sys
import tarfile
import urllib.request
from pathlib import Path

#: AcadRust version pinned in Cargo.toml, and the package's SHA-256 as
#: Cargo.lock records it.
VERSION = "0.6.4"
CRATE_SHA256 = "94862189cab6e282ed0433a801a216cd1a712953813284a48fa81c67223e38f6"
CRATE_URL = f"https://static.crates.io/crates/acadrust/acadrust-{VERSION}.crate"

#: Path in the package's tests/ folder → SHA-256 of the file in the package.
SAMPLES: dict[str, str] = {
    "anonymous-names.dwg": "ea5b55f7e99d2ad412779ef7f3e71ff3bd7f6c4147f6217d936eed02171291b9",
    "datatable/point_object_id.dwg": (
        "9eef9375c77d72881dc202b9fb94b9c414a048dfdfab8374419b44830337c49d"
    ),
    "datatable/point_object_id.dxf": (
        "d143ef14b10d0f071f989614528c25bffa5e8f52521e1b718686da749e3767df"
    ),
    "fixtures/dynamic-blocks/lookup.dwg": (
        "290a8696fff1328f29769dda2c5106fdc9b1f7373d010790271606dbf008df69"
    ),
    "fixtures/dynamic-blocks/point.dwg": (
        "808f0a0d7ad029c6a23fed4fd87323fd03d6938f4feee2d8ab64c2e70e21f232"
    ),
    "fixtures/dynamic-blocks/polar.dwg": (
        "8bdea037b2a59e33be11c5b0267530f0c48123746981007f738affc8d34b67b8"
    ),
    "fixtures/dynamic-blocks/xy.dwg": (
        "044844db69865b22dbe8a9407e3d94abe15af416ad9a03f56765c3e28d7df983"
    ),
    "fixtures/viewports/reference-angle-units.dxf": (
        "cf74c6ac182902bbbb07b289b3ef4712e7951be52bd67f47f81afb730045463e"
    ),
    "fixtures/viewports/reference-clip-activation.dxf": (
        "addc906c7fb62b1f8b26f93423903e8ed72452aed828d3c5da1234f837d198e4"
    ),
    "issue68/r12_mleaderstyle_repro.dxf": (
        "bf332e623b55311095ea861c7f975cccf2f000ac50f10fe5c8460023770b21bc"
    ),
    "mpolygon-fixture.dwg": "13a9a2b46740174f41467603f5baf3b8186c5b6726ab1e43e529ef7c1a2a5cdd",
}

DESTINATION = Path(__file__).resolve().parent.parent / "target" / "corpus" / "acadrust"

#: LibreDWG release whose test data the regression drawings come from.
LIBREDWG_TAG = "0.13.3"
LIBREDWG_URL = f"https://raw.githubusercontent.com/LibreDWG/libredwg/{LIBREDWG_TAG}/test/test-data/"

#: Path in LibreDWG's test-data folder → SHA-256.
LIBREDWG_SAMPLES: dict[str, str] = {
    "r14/Constraints.dwg": "8ac3451b4766ff4c723aa8a4aa08e340db3c7c74457d4cad29a79c134b3fb4d3",
    "2000/Constraints.dwg": "3aeee0f526f5c68cb6aa9532832e04a717a2bfd317d0fdd696c21919724e9280",
    "2004/Constraints.dwg": "df59bb7d1aa251240f45c3eb6b3e945abd651cfd47ce71aa12eec6cc0b4e78cb",
    "2007/Constraints.dwg": "22793d0014cf4d8af5782e6cfd07d7f538e30147296dd34e75957449a3970c74",
    "2010/Constraints.dwg": "bbac6b21d4edd199794a47af639263d0b36a62bcd2d1cce330904800b1ba8fb4",
    "2013/Constraints.dwg": "bdd7458bda6c1464faa5836b8aa85640199fbd01c510fa00e25550c043185429",
    "2018/Constraints.dwg": "6989a481735268caebe99bd64c3376c0e769d5b8f77762ece3920159cc52cc2e",
}

LIBREDWG_DESTINATION = DESTINATION.parent / "libredwg"


def sha256(data: bytes) -> str:
    """Hex SHA-256 of ``data``."""
    return hashlib.sha256(data).hexdigest()


def present(destination: Path, samples: dict[str, str]) -> bool:
    """Whether every sample is already in place and intact."""
    return all(
        (destination / path).is_file() and sha256((destination / path).read_bytes()) == expected
        for path, expected in samples.items()
    )


def fetch_libredwg() -> int:
    """Download LibreDWG's regression drawings; the number that failed."""
    if present(LIBREDWG_DESTINATION, LIBREDWG_SAMPLES):
        print(f"{len(LIBREDWG_SAMPLES)} LibreDWG drawings already in {LIBREDWG_DESTINATION}")
        return 0
    failed = 0
    for path, expected in LIBREDWG_SAMPLES.items():
        with urllib.request.urlopen(LIBREDWG_URL + path, timeout=120) as response:
            data: bytes = response.read()
        if sha256(data) != expected:
            failed += 1
            print(f"{'FAILED':>10}  libredwg {path}: checksum mismatch", file=sys.stderr)
            continue
        target = LIBREDWG_DESTINATION / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
        print(f"{'fetched':>10}  libredwg {path}")
    return failed


def main() -> int:
    """Fetch every sample; non-zero exit if any failed."""
    failed_libredwg = fetch_libredwg()
    if present(DESTINATION, SAMPLES):
        print(f"{len(SAMPLES)}/{len(SAMPLES)} samples already in {DESTINATION}")
        return 1 if failed_libredwg else 0
    with urllib.request.urlopen(CRATE_URL, timeout=120) as response:
        crate: bytes = response.read()
    if sha256(crate) != CRATE_SHA256:
        print(f"acadrust-{VERSION}.crate: checksum mismatch, refusing it", file=sys.stderr)
        return 1
    failed = 0
    prefix = f"acadrust-{VERSION}/tests/"
    with tarfile.open(fileobj=io.BytesIO(crate), mode="r:gz") as archive:
        for path, expected in SAMPLES.items():
            member = archive.extractfile(prefix + path)
            data = member.read() if member is not None else b""
            if sha256(data) != expected:
                failed += 1
                print(f"{'FAILED':>10}  {path}: checksum mismatch", file=sys.stderr)
                continue
            target = DESTINATION / path
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(data)
            print(f"{'extracted':>10}  {path}")
    print(f"{len(SAMPLES) - failed}/{len(SAMPLES)} samples in {DESTINATION}")
    return 1 if failed or failed_libredwg else 0


if __name__ == "__main__":
    sys.exit(main())
