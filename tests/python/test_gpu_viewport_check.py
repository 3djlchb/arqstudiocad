"""The GPU viewport check's overlay comparison (tools/gpu_viewport_check.py,
ADR 0034 D6.4): a blank capture is refused, not passed as the same."""

import importlib.util
from pathlib import Path
from types import ModuleType

import pytest
from PySide6.QtCore import QLineF
from PySide6.QtGui import QColor, QImage, QPainter, QPen
from PySide6.QtWidgets import QApplication

TOOL = Path(__file__).resolve().parents[2] / "tools" / "gpu_viewport_check.py"

#: Not the viewport's background: the comparison measures it in the image.
PAPER = QColor(230, 225, 210)


def load_tool() -> ModuleType:
    spec = importlib.util.spec_from_file_location("gpu_viewport_check", TOOL)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def picture(path: Path, lines: list[QLineF], fill: QColor = PAPER) -> Path:
    image = QImage(120, 80, QImage.Format.Format_RGB32)
    image.fill(fill)
    painter = QPainter(image)
    painter.setPen(QPen(QColor(20, 60, 160), 2))
    for line in lines:
        painter.drawLine(line)
    painter.end()
    assert image.save(str(path))
    return path


ACROSS = QLineF(5, 10, 115, 70)
DOWN = QLineF(60, 5, 60, 75)


def test_the_same_drawing_compares_equal(qapp: QApplication, tmp_path: Path) -> None:
    tool = load_tool()
    cpu = picture(tmp_path / "cpu.png", [ACROSS, DOWN])
    gpu = picture(tmp_path / "gpu.png", [ACROSS, DOWN])
    assert tool.compare(gpu, cpu) == 0


def test_a_missing_line_is_counted(qapp: QApplication, tmp_path: Path) -> None:
    tool = load_tool()
    cpu = picture(tmp_path / "cpu.png", [ACROSS, DOWN])
    gpu = picture(tmp_path / "gpu.png", [ACROSS])
    assert tool.compare(gpu, cpu) > 50, "the vertical line, against a light background"


def test_a_blank_capture_is_refused(qapp: QApplication, tmp_path: Path) -> None:
    # A black window: what reading back a DirectX 12 window gives on
    # Windows, and what OpenGL on the Intel UHD 630 shows on screen.
    tool = load_tool()
    cpu = picture(tmp_path / "cpu.png", [ACROSS, DOWN])
    # Windows 11 rounds the window's corners: a few pixels of the desktop.
    black = QImage(120, 80, QImage.Format.Format_RGB32)
    black.fill(QColor(0, 0, 0))
    for x, y in ((0, 0), (1, 0), (0, 1), (119, 0), (0, 79), (119, 79)):
        black.setPixelColor(x, y, QColor(90, 120, 200))
    black.save(str(tmp_path / "black.png"))
    with pytest.raises(ValueError, match="GPU image is blank"):
        tool.compare(tmp_path / "black.png", cpu)
    with pytest.raises(ValueError, match="CPU image is blank"):
        tool.compare(cpu, tmp_path / "black.png")


def test_images_that_cannot_be_compared(qapp: QApplication, tmp_path: Path) -> None:
    tool = load_tool()
    cpu = picture(tmp_path / "cpu.png", [ACROSS])
    with pytest.raises(ValueError, match="missing"):
        tool.compare(tmp_path / "none.png", cpu)
    small = QImage(60, 40, QImage.Format.Format_RGB32)
    small.fill(PAPER)
    small.save(str(tmp_path / "small.png"))
    with pytest.raises(ValueError, match="size"):
        tool.compare(tmp_path / "small.png", cpu)
