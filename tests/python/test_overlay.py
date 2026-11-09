"""The overlay's description (ADR 0034 D4) and its QPainter painter."""

from PySide6.QtCore import Qt
from PySide6.QtGui import QColor, QFont, QImage, QPainter
from pytestqt.qtbot import QtBot

from arqstudiocad.ui.overlay import (
    CIRCLE_SEGMENTS,
    Fill,
    Label,
    Overlay,
    Stroke,
    marker,
    paint,
    rectangle,
    to_core,
)

BLUE = (30, 144, 255, 255)
DARK = QColor(33, 40, 48)


def painted(overlay: Overlay, width: int = 100, height: int = 100) -> QImage:
    image = QImage(width, height, QImage.Format.Format_RGB32)
    image.fill(DARK)
    painter = QPainter(image)
    paint(painter, overlay)
    painter.end()
    return image


def rgb(image: QImage, x: int, y: int) -> tuple[int, int, int]:
    c = image.pixelColor(x, y)
    return (c.red(), c.green(), c.blue())


def test_every_snap_has_its_glyph() -> None:
    kinds = ("END", "MID", "CEN", "QUA", "INT", "PER", "TAN", "NEA")
    glyphs = {kind: marker(kind, (50.0, 50.0), 6.0, BLUE, 2.0) for kind in kinds}
    assert all(glyphs.values()), "something for every kind"
    assert len({str(g) for g in glyphs.values()}) == len(kinds), "all different"
    (box,) = glyphs["END"]
    assert box.closed and box.points == rectangle((44.0, 44.0), (56.0, 56.0))
    (ring,) = glyphs["CEN"]
    assert ring.closed and len(ring.points) == CIRCLE_SEGMENTS
    assert len(glyphs["INT"]) == 2, "a cross of two strokes"


def test_the_cpu_paints_a_crisp_stroke_on_its_pixels() -> None:
    image = painted(Overlay([Stroke(((10.0, 30.0), (20.0, 30.0)), BLUE, crisp=True)]))
    assert rgb(image, 15, 30) == BLUE[:3]
    assert rgb(image, 15, 29) == rgb(image, 15, 31) == rgb(image, 5, 5)


def test_the_cpu_blends_a_translucent_fill() -> None:
    image = painted(Overlay([Fill((10.0, 10.0), (30.0, 20.0), (30, 144, 255, 40))]))
    want = [
        round(d + (b - d) * 40 / 255)
        for d, b in zip((DARK.red(), DARK.green(), DARK.blue()), BLUE[:3], strict=True)
    ]
    got = rgb(image, 20, 15)
    assert all(abs(g - w) <= 2 for g, w in zip(got, want, strict=True)), (got, want)
    assert rgb(image, 5, 15) == (DARK.red(), DARK.green(), DARK.blue())


def test_the_gpu_gets_tagged_items_in_order_and_texts_as_images(qtbot: QtBot) -> None:
    overlay = Overlay(
        [
            Fill((1.0, 2.0), (3.0, 4.0), (1, 2, 3, 4)),
            Stroke(((0.0, 0.0), (5.0, 5.0)), BLUE, 2.0, dashed=True, closed=False, crisp=True),
        ],
        [Label((10.0, 30.0), "Endpoint", (255, 200, 0, 255))],
    )
    font = QFont()
    items, labels = to_core(overlay, font, 2.0)
    assert items == [
        ("fill", (1.0, 2.0), (3.0, 4.0), (1, 2, 3, 4)),
        ("stroke", [(0.0, 0.0), (5.0, 5.0)], BLUE, 2.0, True, False, True),
    ]
    ((x, y, width, height, pixels),) = labels
    assert x == 20, "physical pixels: twice the logical ones"
    assert y < 60, "the top-left corner, above the baseline"
    assert width > 20 and height > 10
    assert len(pixels) == width * height * 4
    alphas = pixels[3::4]
    assert max(alphas) == 255 and min(alphas) == 0, "the letters on transparency"


def test_an_empty_text_is_left_out(qtbot: QtBot) -> None:
    _, labels = to_core(Overlay([], [Label((0.0, 10.0), "", BLUE)]), QFont(), 1.0)
    assert labels == []


def test_painting_leaves_the_painter_as_it_was() -> None:
    image = QImage(10, 10, QImage.Format.Format_RGB32)
    painter = QPainter(image)
    painter.setRenderHint(QPainter.RenderHint.Antialiasing, False)
    paint(painter, Overlay([Stroke(((0.0, 0.0), (9.0, 9.0)), BLUE)]))
    assert not painter.testRenderHint(QPainter.RenderHint.Antialiasing)
    assert painter.pen().style() != Qt.PenStyle.DashLine
    painter.end()
