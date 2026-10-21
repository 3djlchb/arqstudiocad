"""Damaged drawings for the tests (ADR 0017 D5.1, D5.4), made from a
drawing saved here, never kept as files."""

from pathlib import Path

from arqstudiocad.core import CadApplication

LINES = 20


def _saved(path: Path) -> Path:
    document = CadApplication().create_document("x")
    for i in range(LINES):
        document.add_line((i, 0), (i, 10))
    document.save(path)
    document.close()
    return path


def recovered_dxf(path: Path) -> Path:
    """A DXF whose sixth LINE has an invalid group code: the strict read
    fails, recovery skips that record and reads the other lines."""
    lines = _saved(path).read_text(encoding="utf-8").split("\n")
    entities = lines.index("ENTITIES")
    starts = [
        i for i in range(entities, len(lines)) if lines[i] == "LINE" and lines[i - 1].strip() == "0"
    ]
    lines[starts[5] + 1] = "zz"
    path.write_text("\n".join(lines), encoding="utf-8")
    return path


def unreadable_dwg(path: Path) -> Path:
    """A DWG cut in half: nothing can be recovered from it."""
    data = _saved(path).read_bytes()
    path.write_bytes(data[: len(data) // 2])
    return path
