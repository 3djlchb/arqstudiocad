"""Builds the common corpus of the DWG/DXF comparison (spike).

Nothing here is committed: the corpus lives outside the repository.

    python3 prepare_corpus.py <corpus-dir> <libredwg-source-dir> \
        <acadrust-crate-dir> [local-drawings-dir]

Groups:

- ``libredwg/``  LibreDWG's own test drawings (R1.4 to 2018, one per entity
  kind, constraints, surfaces…), copied from its source tree (GPL-3.0+);
- ``codec/``     the sample drawings shipped with the AcadRust crate (MPL-2.0),
  dynamic blocks among them;
- ``local/``     a link to the maintainer's own drawings, never copied;
- ``generated/`` a large 2D drawing written here as plain DXF text, so no
  library under test made it;
- ``damaged/``   truncated and byte-flipped copies of a few drawings, for
  error tolerance.
"""

from __future__ import annotations

import math
import random
import shutil
import sys
from pathlib import Path


def copy_tree(source: Path, target: Path) -> int:
    count = 0
    for path in sorted(source.rglob("*")):
        if path.suffix.lower() in {".dwg", ".dxf"} and path.is_file():
            destination = target / path.relative_to(source)
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(path, destination)
            count += 1
    return count


def big_dxf(path: Path, entities: int) -> None:
    """An R2000 ASCII DXF: 50 layers, lines, circles, arcs, polylines and
    texts on a grid, with survey-size coordinates."""
    rng = random.Random(2026)
    out = ["0", "SECTION", "2", "HEADER", "9", "$ACADVER", "1", "AC1015", "0", "ENDSEC"]
    out += ["0", "SECTION", "2", "TABLES", "0", "TABLE", "2", "LAYER", "70", "50"]
    for n in range(50):
        out += [
            "0",
            "LAYER",
            "2",
            f"CAPA-{n:02d}",
            "70",
            "0",
            "62",
            str(1 + n % 7),
            "6",
            "CONTINUOUS",
        ]
    out += ["0", "ENDTAB", "0", "ENDSEC", "0", "SECTION", "2", "ENTITIES"]
    x0, y0 = 500_000.0, 9_977_000.0
    for n in range(entities):
        layer = f"CAPA-{n % 50:02d}"
        x = x0 + (n % 1000) * 10.0 + rng.random()
        y = y0 + (n // 1000) * 10.0 + rng.random()
        kind = n % 5
        if kind == 0:
            out += ["0", "LINE", "8", layer, "10", f"{x:.4f}", "20", f"{y:.4f}", "30", "0"]
            out += ["11", f"{x + 7:.4f}", "21", f"{y + 3:.4f}", "31", "0"]
        elif kind == 1:
            out += ["0", "CIRCLE", "8", layer, "10", f"{x:.4f}", "20", f"{y:.4f}", "30", "0"]
            out += ["40", f"{1 + rng.random() * 3:.4f}"]
        elif kind == 2:
            start = rng.random() * 360
            out += ["0", "ARC", "8", layer, "10", f"{x:.4f}", "20", f"{y:.4f}", "30", "0"]
            out += ["40", "2.5", "50", f"{start:.4f}", "51", f"{(start + 120) % 360:.4f}"]
        elif kind == 3:
            out += ["0", "LWPOLYLINE", "8", layer, "90", "4", "70", "1"]
            for dx, dy in ((0, 0), (5, 0), (5, 4), (0, 4)):
                out += ["10", f"{x + dx:.4f}", "20", f"{y + dy:.4f}"]
        else:
            angle = math.degrees(rng.random())
            out += ["0", "TEXT", "8", layer, "10", f"{x:.4f}", "20", f"{y:.4f}", "30", "0"]
            out += ["40", "1.5", "1", f"T{n}", "50", f"{angle:.4f}"]
    out += ["0", "ENDSEC", "0", "EOF"]
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("\n".join(out) + "\n", encoding="ascii")


def damage(source: Path, target_dir: Path) -> int:
    """Three damaged copies of `source`: cut at 50 % and at 90 %, and 32
    bytes flipped past the header."""
    data = source.read_bytes()
    target_dir.mkdir(parents=True, exist_ok=True)
    stem = source.stem.replace(" ", "_")
    (target_dir / f"{stem}-cut50{source.suffix}").write_bytes(data[: len(data) // 2])
    (target_dir / f"{stem}-cut90{source.suffix}").write_bytes(data[: len(data) * 9 // 10])
    flipped = bytearray(data)
    rng = random.Random(len(data))
    for _ in range(32):
        at = rng.randrange(min(len(data) - 1, 256), len(data))
        flipped[at] ^= 0xFF
    (target_dir / f"{stem}-flipped{source.suffix}").write_bytes(bytes(flipped))
    return 3


def main() -> None:
    corpus, libredwg_src, acadrust_crate = (Path(a) for a in sys.argv[1:4])
    local = Path(sys.argv[4]) if len(sys.argv) > 4 else None
    corpus.mkdir(parents=True, exist_ok=True)
    counts = {
        "libredwg": copy_tree(libredwg_src / "test" / "test-data", corpus / "libredwg"),
        "codec": copy_tree(acadrust_crate / "tests", corpus / "codec"),
    }
    big_dxf(corpus / "generated" / "grande-100k.dxf", 100_000)
    counts["generated"] = 1
    victims = [
        corpus / "libredwg" / "example_2018.dwg",
        corpus / "libredwg" / "example_2000.dwg",
        corpus / "codec" / "fixtures" / "dynamic-blocks" / "lookup.dwg",
    ]
    counts["damaged"] = sum(damage(v, corpus / "damaged") for v in victims if v.is_file())
    if local is not None and local.is_dir():
        link = corpus / "local"
        if not link.exists():
            link.symlink_to(local, target_is_directory=True)
        counts["local"] = len([p for p in local.rglob("*") if p.suffix.lower() in {".dwg", ".dxf"}])
    for group, n in counts.items():
        print(f"{group:10s} {n}")


if __name__ == "__main__":
    main()
