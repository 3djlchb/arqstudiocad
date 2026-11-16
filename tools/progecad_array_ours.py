"""Our side of the rectangular ARRAY comparison with ProgeCAD (milestone
02.14, docs/testing/array-rectangular-windows.md): the same synthetic
sources and parameters as `arq:compare-array` in
tools/progecad_command_compare.lsp, through our core's rectangular_array,
each result saved as DWG and DXF for ProgeCAD to reopen.

    uv run python tools/progecad_array_ours.py <folder>
"""

import math
import sys
from pathlib import Path

from arqstudiocad._core import CadApplication, CadColor, CadDocument

#: name, sources, rows, columns, row spacing, column spacing, angle in degrees
CASES: list[tuple[str, str, int, int, float, float, float]] = [
    ("C1-basic-3x4", "line", 3, 4, 2.0, 3.0, 0.0),
    ("C2-negative-3x4", "line", 3, 4, -2.0, -3.0, 0.0),
    ("C3-snapang30-3x4", "line", 3, 4, 2.0, 3.0, 30.0),
    ("C4-many-sources-2x3", "many", 2, 3, 4.0, 15.0, 0.0),
    ("C5-one-row-1x5", "line", 1, 5, 1.0, 3.0, 0.0),
    ("C6-one-column-5x1", "line", 5, 1, 2.0, 1.0, 0.0),
    ("C7-elevated-2x2", "elevated", 2, 2, 3.0, 3.0, 0.0),
    ("C8-zero-spacing-3x3", "line", 3, 3, 0.0, 0.0, 0.0),
    ("C9-over-limit-317x316", "line", 317, 316, 2.0, 3.0, 0.0),
]


def sources(doc: CadDocument, kind: str) -> list[int]:
    """The sources ProgeCAD's run builds for ``kind``."""
    if kind == "line":
        return [doc.add_line((0, 0), (1, 0.5))[0]]
    if kind == "many":
        layer, _ = doc.create_layer("ARRAY-TEST")
        line = doc.add_line((0, 0), (1, 0.5))[0]
        doc.set_entity_layer([line], layer)
        doc.set_entity_color([line], CadColor.from_index(1))
        circle = doc.add_circle((5, 5), 1.0)[0]
        arc = doc.add_arc((8, 1), 1.0, 0.0, math.pi / 2)[0]
        square = [((10.0, 0.0), 0.0), ((12.0, 0.0), 0.0), ((12.0, 2.0), 0.0), ((10.0, 2.0), 0.0)]
        poly = doc.add_polyline(square, closed=True, width=0.25)[0]
        return [line, circle, arc, poly]
    if kind == "elevated":
        square = [((0.0, 0.0), 0.0), ((1.0, 0.0), 0.0), ((1.0, 1.0), 0.0), ((0.0, 1.0), 0.0)]
        return [doc.add_polyline(square, closed=True, elevation=2.0)[0]]
    raise ValueError(kind)


def main() -> None:
    out = Path(sys.argv[1])
    out.mkdir(parents=True, exist_ok=True)
    app = CadApplication()
    for name, kind, rows, cols, row_spacing, col_spacing, degrees in CASES:
        doc = app.create_document(name)
        ids = sources(doc, kind)
        try:
            new, _ = doc.rectangular_array(
                ids, rows, cols, row_spacing, col_spacing, math.radians(degrees)
            )
        except Exception as error:  # refused: written down, nothing saved
            print(f"{name}: refused: {type(error).__name__}: {error}")
            continue
        for ext in ("dwg", "dxf"):
            doc.save(out / f"ours-{name}.{ext}")
        print(f"{name}: {len(new)} new, {len(doc.get_entities())} in all; saved")


if __name__ == "__main__":
    main()
