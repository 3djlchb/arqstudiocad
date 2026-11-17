"""World XYZ orientation and its shared CPU/GPU viewport overlay."""

import math

import pytest
from PySide6.QtGui import QColor, QFont, QImage, QPainter
from pytestqt.qtbot import QtBot

from arqstudiocad.core import CadApplication, CadView
from arqstudiocad.ui.overlay import Stroke, paint, to_core
from arqstudiocad.ui.viewport import Viewport
from arqstudiocad.ui.world_axes import world_axes_overlay

A = 1 / math.sqrt(2)
B = 1 / math.sqrt(6)
C = 2 / math.sqrt(6)


@pytest.mark.parametrize(
    ("name", "expected"),
    [
        ("top", [(1, 0), (0, -1), (0, 0)]),
        ("bottom", [(-1, 0), (0, -1), (0, 0)]),
        ("front", [(1, 0), (0, 0), (0, -1)]),
        ("back", [(-1, 0), (0, 0), (0, -1)]),
        ("left", [(0, 0), (-1, 0), (0, -1)]),
        ("right", [(0, 0), (1, 0), (0, -1)]),
        ("swiso", [(A, -B), (-A, -B), (0, -C)]),
        ("seiso", [(A, B), (A, -B), (0, -C)]),
        ("neiso", [(-A, B), (A, B), (0, -C)]),
        ("nwiso", [(-A, -B), (-A, B), (0, -C)]),
    ],
)
def test_projected_world_axes_follow_every_view(
    name: str,
    expected: list[tuple[float, float]],
) -> None:
    document = CadApplication().create_document("axes")
    view = CadView(document, 800, 600)
    view.view = name
    origin, axes = view.world_axes
    assert origin == pytest.approx(view.plane_to_screen(0, 0))
    for actual, want in zip(axes, expected, strict=True):
        assert actual == pytest.approx(want, abs=1e-12)
    view.pan(17, -23)
    view.zoom_at(200, 150, 2)
    new_origin, new_axes = view.world_axes
    assert new_origin == pytest.approx(view.plane_to_screen(0, 0))
    assert new_origin != origin
    assert new_axes == axes, "zoom never changes indicator size"
    assert document.get_entities() == [], "an overlay is never drawing geometry"
    assert not document.is_modified and not document.can_undo


@pytest.mark.parametrize("ratio", [1.0, 2.0])
def test_viewport_axes_stay_at_world_origin_through_pan_zoom_and_resize(
    qtbot: QtBot,
    monkeypatch: pytest.MonkeyPatch,
    ratio: float,
) -> None:
    document = CadApplication().create_document("axes")
    viewport = Viewport(document)
    qtbot.addWidget(viewport)
    viewport.resize(800, 600)
    viewport.view.resize(round(800 * ratio), round(600 * ratio))
    monkeypatch.setattr(viewport, "devicePixelRatioF", lambda: ratio)
    for name in ("top", "swiso", "front"):
        viewport.view.view = name
        ox, oy = viewport.view.world_axes[0]
        viewport.view.pan(400 * ratio - ox, 300 * ratio - oy)
        for pan in (0, 35):
            viewport.view.pan(pan * ratio, -20 * ratio)
            viewport.view.zoom_at(200 * ratio, 150 * ratio, 1.25)
            out = viewport.overlay()
            assert [label.text for label in out.labels] == ["X", "Y", "Z"]
            origin = viewport.view.plane_to_screen(0, 0)
            want = (origin[0] / ratio, origin[1] / ratio)
            shafts = [s for s in out.items if isinstance(s, Stroke) and len(s.points) == 2]
            assert all(s.points[0] == pytest.approx(want) for s in shafts)
            assert len(shafts) == (3 if name == "swiso" else 2)
            assert max(math.dist(*s.points) for s in shafts) == pytest.approx(38)
    viewport.view.pan(10000, 0)
    assert not viewport.overlay().items and not viewport.overlay().labels
    viewport.resize(1000, 700)
    viewport.view.resize(round(1000 * ratio), round(700 * ratio))
    ox, oy = viewport.view.plane_to_screen(0, 0)
    viewport.view.pan(500 * ratio - ox, 350 * ratio - oy)
    restored = viewport.overlay()
    shafts = [s for s in restored.items if isinstance(s, Stroke) and len(s.points) == 2]
    assert shafts and all(s.points[0] == pytest.approx((500, 350)) for s in shafts)


@pytest.mark.parametrize("background", [(33, 40, 48), (255, 255, 255)])
def test_colored_axes_are_painted_and_serialized_for_both_paths(
    qtbot: QtBot,
    background: tuple[int, int, int],
) -> None:
    out = world_axes_overlay((100, 100), [(1, 0), (0, -1), (0, 0)], 200, 200, background)
    colors: list[tuple[int, int, int, int]] = [
        (230, 56, 56, 255),
        (56, 153 if background[0] == 255 else 217, 56, 255),
        (56, 115, 230, 255),
    ]
    assert [label.color for label in out.labels] == colors
    image = QImage(200, 200, QImage.Format.Format_RGB32)
    image.fill(QColor(*background))
    painter = QPainter(image)
    paint(painter, out)
    painter.end()
    for position, color in zip([(120, 100), (100, 80), (103, 100)], colors, strict=True):
        got = image.pixelColor(*position)
        assert (
            max(
                abs(a - b)
                for a, b in zip((got.red(), got.green(), got.blue()), color[:3], strict=True)
            )
            < 30
        )
    items, labels = to_core(out, QFont(), 2)
    assert len(items) == 5 and len(labels) == 3
    assert {item[2] for item in items} == set(colors)
    assert all(len(pixels) == w * h * 4 and max(pixels[3::4]) > 0 for _, _, w, h, pixels in labels)
