"""Writes the hand-made hostile drawings (ADR 0037 D2): the files
`crates/arq-core/tests/hostile_files.rs` opens and the fuzzers start from.

    uv run python tools/make_hostile_drawings.py crates/arq-core/tests/hostile

Broken on purpose: empty or cut files, numbers that are not numbers, NaN
and infinity, absurd counts, blocks that insert themselves or nest a
thousand deep, and DWG headers followed by zeros or noise.
"""

import random
import sys
from pathlib import Path

#: A DXF group: its code and its value, each on a line of its own.
Pair = tuple[str, str]
Pairs = tuple[Pair, ...]


def dxf(*pairs: Pair) -> bytes:
    return "".join(f"{code}\n{value}\n" for code, value in pairs).encode("ascii")


def entities(*items: Pair) -> bytes:
    return dxf(("0", "SECTION"), ("2", "ENTITIES"), *items, ("0", "ENDSEC"), ("0", "EOF"))


def blocks_and_entities(blocks: Pairs, ents: Pairs) -> bytes:
    return dxf(
        ("0", "SECTION"),
        ("2", "BLOCKS"),
        *blocks,
        ("0", "ENDSEC"),
        ("0", "SECTION"),
        ("2", "ENTITIES"),
        *ents,
        ("0", "ENDSEC"),
        ("0", "EOF"),
    )


def block(name: str, *items: Pair) -> Pairs:
    head: Pairs = (("0", "BLOCK"), ("8", "0"), ("2", name), ("70", "0"))
    base: Pairs = (("10", "0"), ("20", "0"), ("30", "0"), ("3", name))
    return (*head, *base, *items, ("0", "ENDBLK"), ("8", "0"))


def insert(name: str, *extra: Pair) -> Pairs:
    at: Pairs = (("10", "0"), ("20", "0"), ("30", "0"))
    return (("0", "INSERT"), ("8", "0"), ("2", name), *at, *extra)


def line(x1: str, y1: str, x2: str, y2: str) -> Pairs:
    start: Pairs = (("10", x1), ("20", y1), ("30", "0"))
    end: Pairs = (("11", x2), ("21", y2), ("31", "0"))
    return (("0", "LINE"), ("8", "0"), *start, *end)


def drawings() -> dict[str, bytes]:
    unit = block("UNIT", *line("0", "0", "1", "1"))
    array: Pairs = (("70", "100000"), ("71", "100000"), ("44", "1"), ("45", "1"))
    polyline: Pairs = (
        ("0", "LWPOLYLINE"),
        ("8", "0"),
        ("90", "4294967295"),
        ("70", "0"),
        ("10", "0"),
        ("20", "0"),
        ("10", "1"),
        ("20", "1"),
    )
    files = {
        "empty.dxf": b"",
        "eof-only.dxf": dxf(("0", "EOF")),
        "cut-inside-an-entity.dxf": b"0\nSECTION\n2\nENTITIES\n0\nLINE\n8\n0\n10\n",
        "not-numbers.dxf": entities(*line("abc", "1e999", "-", "0x10")),
        "nan-and-infinity.dxf": entities(
            *line("nan", "inf", "-inf", "1e308"), *line("1e308", "1e308", "-1e308", "-1e308")
        ),
        "huge-vertex-count.dxf": entities(*polyline),
        "block-inserting-itself.dxf": blocks_and_entities(
            block("SELF", *line("0", "0", "1", "1"), *insert("SELF")), insert("SELF")
        ),
        "blocks-inserting-each-other.dxf": blocks_and_entities(
            (*block("A", *insert("B")), *block("B", *insert("A"))), insert("A")
        ),
        "huge-insert-array.dxf": blocks_and_entities(unit, insert("UNIT", *array)),
        "group-code-garbage.dxf": b"0\nSECTION\nxyz\n\x00\x01\x02\n99999999999999999999\nLINE\n",
        "empty.dwg": b"",
        "version-only.dwg": b"AC1032",
        "header-then-zeros.dwg": b"AC1032" + bytes(4096),
    }
    rng = random.Random(20261111)
    for version in ("AC1015", "AC1018", "AC1032"):
        noise = bytes(rng.randrange(256) for _ in range(8192))
        files[f"header-then-noise-{version}.dwg"] = version.encode() + noise
    # Nested a thousand deep, for recursion in the reader or the drawing.
    deep: Pairs = block("B0", *line("0", "0", "1", "1"))
    for i in range(1, 1000):
        deep = (*deep, *block(f"B{i}", *insert(f"B{i - 1}")))
    files["blocks-nested-a-thousand-deep.dxf"] = blocks_and_entities(deep, insert("B999"))
    return files


def main() -> None:
    out = Path(sys.argv[1])
    out.mkdir(parents=True, exist_ok=True)
    files = drawings()
    for name, data in files.items():
        (out / name).write_bytes(data)
    print(len(files), "files")


if __name__ == "__main__":
    main()
