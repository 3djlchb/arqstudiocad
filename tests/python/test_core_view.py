"""CadView: the document rendered on the CPU for the viewport (ADR 0013, D5.3).

Pixels are found through the view's own camera (to_screen), and checked
against hand-picked world points on the drawn geometry.
"""

import math
import time
from collections.abc import Callable
from pathlib import Path

import pytest
from gil_helpers import big_dxf, share_of_time_other_threads_ran

from arqstudiocad.core import (
    CadApplication,
    CadBounds2,
    CadColor,
    CadDocument,
    CadView,
    DocumentNotFoundError,
)

DARK = (33, 40, 48)


@pytest.fixture
def doc() -> CadDocument:
    return CadApplication().create_document("Plano1")


def pixel(view: CadView, image: bytes, x: float, y: float) -> tuple[int, int, int]:
    i = (int(y) * view.width + int(x)) * 4
    return image[i], image[i + 1], image[i + 2]


def colors_near(view: CadView, image: bytes, wx: float, wy: float) -> set[tuple[int, int, int]]:
    """The colors of the pixels at and next to world (wx, wy)."""
    sx, sy = view.to_screen(wx, wy)
    return {
        pixel(view, image, sx + dx, sy + dy)
        for dx in (-1, 0, 1)
        for dy in (-1, 0, 1)
        if 0 <= sx + dx < view.width and 0 <= sy + dy < view.height
    }


def any_pixel_near(
    view: CadView,
    image: bytes,
    wx: float,
    wy: float,
    test: Callable[[tuple[int, int, int]], bool],
) -> bool:
    """Whether a pixel at or next to world (wx, wy) passes `test`. Colors are
    tested by kind (red dominates, dark on light), not exact values:
    antialiasing blends a line that is not pixel-aligned with the background."""
    return any(test(c) for c in colors_near(view, image, wx, wy))


def lit_near(view: CadView, image: bytes, wx: float, wy: float) -> bool:
    """Whether a pixel at or next to world (wx, wy) is drawn."""
    return colors_near(view, image, wx, wy) != {view.background}


def test_an_empty_document(doc: CadDocument) -> None:
    view = CadView(doc, 120, 80)
    assert (view.width, view.height, view.shape_count, view.bounds) == (120, 80, 0, None)
    assert not view.zoom_extents()
    image = view.render()
    assert len(image) == 120 * 80 * 4
    assert image == bytes([*DARK, 255]) * (120 * 80), "only background"
    assert view.document == doc
    assert repr(view) == "CadView(120x80, 0 shapes)"


def test_it_shows_the_whole_drawing(doc: CadDocument) -> None:
    doc.add_line((0, 0), (100, 50))
    red, _ = doc.add_circle((80, 10), 5)
    doc.set_entity_color([red], CadColor.from_index(1))
    view = CadView(doc, 200, 100)
    assert view.bounds == CadBounds2((0, 0), (100, 50))
    image = view.render()
    assert lit_near(view, image, 50, 25), "middle of the line"
    assert any_pixel_near(view, image, 85, 10, lambda c: c[0] - max(c[1], c[2]) >= 80), (
        "the circle's right side is red"
    )
    assert not lit_near(view, image, 20, 45), "empty corner"


def test_it_follows_changes(doc: CadDocument) -> None:
    line, _ = doc.add_line((0, 0), (100, 0))
    view = CadView(doc, 200, 100)
    assert view.shape_count == 1

    moved = doc.move_entities([line], (0, 20))
    assert view.update(moved)
    image = view.render()
    assert lit_near(view, image, 50, 20) and not lit_near(view, image, 50, 0)

    circle, made = doc.add_circle((50, -20), 5)
    view.update(made)
    assert view.shape_count == 2

    zero = doc.current_layer.id
    view.update(doc.set_layer_visible(zero, False))
    assert view.shape_count == 0, "off layers are not drawn"
    view.update(doc.set_layer_visible(zero, True))
    view.update(doc.delete_entities([line, circle]))
    assert view.shape_count == 0
    assert not view.update(doc.delete_entities([])), "nothing changed"


def test_camera_controls(doc: CadDocument) -> None:
    doc.add_line((0, 0), (10, 10))
    view = CadView(doc, 100, 100)
    scale = view.scale
    world = view.to_world(30, 70)
    view.zoom_at(30, 70, 2.0)
    assert view.scale == pytest.approx(scale * 2)
    assert view.to_world(30, 70) == pytest.approx(world), "the point under the cursor stays"
    grabbed = view.to_world(10, 10)
    view.pan(15, -5)
    assert view.to_world(25, 5) == pytest.approx(grabbed), "the drawing follows the drag"
    view.resize(64, 48)
    assert (view.width, view.height, len(view.render())) == (64, 48, 64 * 48 * 4)
    assert view.zoom_extents()
    assert view.center == pytest.approx((5, 5))
    view.background = (250, 250, 250)
    assert view.background == (250, 250, 250)
    image = view.render()
    assert pixel(view, image, 0, 0) == (250, 250, 250)
    assert any_pixel_near(view, image, 5, 5, lambda c: max(c) <= 128), "ACI 7 is dark on light"


def test_a_closed_document(doc: CadDocument) -> None:
    line, _ = doc.add_line((0, 0), (1, 1))
    view = CadView(doc, 50, 50)
    changes = doc.move_entities([line], (1, 0))
    doc.close()
    with pytest.raises(DocumentNotFoundError):
        view.update(changes)
    with pytest.raises(DocumentNotFoundError):
        view.rebuild()
    assert len(view.render()) == 50 * 50 * 4, "it still shows what it had"
    with pytest.raises(DocumentNotFoundError):
        CadView(doc, 10, 10)


def test_rendering_releases_the_gil(tmp_path: Path) -> None:
    doc = CadApplication().open_document(big_dxf(tmp_path / "grande.dxf", 200_000))
    view = CadView(doc, 2000, 2000)
    start = time.perf_counter()
    view.render()
    once = time.perf_counter() - start
    # Fast machines (CI runners among them) draw it in well under 0.1 s:
    # render as many times as it takes to measure for about half a second.
    times = max(1, math.ceil(0.5 / once))

    def render_repeatedly() -> None:
        for _ in range(times):
            view.render()

    share, elapsed = share_of_time_other_threads_ran(render_repeatedly)
    doc.close()
    assert elapsed > 0.1, f"{times} renders took {elapsed:.3f} s: too short to tell"
    assert share > 0.3, f"other threads ran {share:.0%} of the time while rendering"
