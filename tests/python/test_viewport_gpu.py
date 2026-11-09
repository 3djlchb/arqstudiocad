"""The GPU viewport (ADR 0034 D6.1): CadView presenting into a native window.

The unit tests run on pytest's offscreen platform, where there is no X11
window. The window test runs a real viewport on X11 in a child process: it
needs a display and a graphics adapter, says why it skipped without them,
and fails instead when ``ARQ_GPU_TESTS=require`` (the reference machine).
"""

import json
import os
import subprocess
import sys
import textwrap
from pathlib import Path

import pytest
from PySide6.QtCore import Qt
from PySide6.QtGui import QColor, QImage
from PySide6.QtWidgets import QApplication
from pytestqt.qtbot import QtBot

from arqstudiocad.core import CadApplication, CadView
from arqstudiocad.ui.viewport import GPU_VIEWPORT_ENV, Viewport

REQUIRED = os.environ.get("ARQ_GPU_TESTS") == "require"


def view() -> CadView:
    document = CadApplication().create_document("Plano")
    document.add_line((0, 0), (100, 50))
    return CadView(document, 200, 100)


def test_attaching_needs_a_known_backend_and_a_real_window() -> None:
    v = view()
    with pytest.raises(ValueError, match="unsupported GPU backend"):
        v.attach_window("glide", 1, 1)
    with pytest.raises(RuntimeError, match="X11"):
        v.attach_window("opengl", 0, 1)
    assert v.gpu is None, "nothing attached after a failure"
    with pytest.raises(RuntimeError, match="no window attached"):
        v.present()
    v.detach_window()  # nothing to detach: no error
    assert len(v.render()) == 200 * 100 * 4, "the CPU keeps rendering"


def test_without_x11_the_viewport_draws_on_the_cpu(
    qtbot: QtBot, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setenv(GPU_VIEWPORT_ENV, "opengl")
    document = CadApplication().create_document("Plano")
    document.add_line((0, 0), (100, 50))
    viewport = Viewport(document)
    qtbot.addWidget(viewport)
    viewport.resize(300, 200)
    viewport.show()
    qtbot.waitExposed(viewport)
    QApplication.processEvents()

    assert viewport.gpu is None, "offscreen Qt has no X11 window"
    assert not viewport.testAttribute(Qt.WidgetAttribute.WA_PaintOnScreen), "Qt paints it again"
    image = viewport.grab().toImage()
    background = viewport.view.background
    drawn = 0
    for y in range(0, image.height(), 3):
        for x in range(0, image.width(), 3):
            color = image.pixelColor(x, y)
            drawn += (color.red(), color.green(), color.blue()) != background
    assert drawn > 0, "the CPU drew the line"


WINDOW_SCRIPT = textwrap.dedent(
    """
    import json, sys, time
    from PySide6.QtCore import Qt
    from PySide6.QtGui import QGuiApplication
    from PySide6.QtWidgets import QApplication
    from arqstudiocad.core import CadApplication
    from arqstudiocad.ui.viewport import Viewport

    app = QApplication([])
    document = CadApplication().create_document("Plano")
    for i in range(40):
        document.add_line((i * 5.0, 0.0), (i * 5.0 + 3.0, 60.0))
    document.add_circle((100.0, 30.0), 25.0)
    viewport = Viewport(document)
    # Grabbed from the screen: kept above any other window on the desktop,
    # and deaf to the real mouse, which would move the crosshair.
    viewport.setWindowFlag(Qt.WindowType.WindowStaysOnTopHint)
    viewport.setAttribute(Qt.WidgetAttribute.WA_TransparentForMouseEvents)
    viewport.resize(400, 300)
    viewport.show()
    viewport.raise_()
    for _ in range(30):
        app.processEvents()
        time.sleep(0.02)
    result = {"gpu": viewport.gpu}

    def grab():
        viewport.repaint()
        for _ in range(3):
            app.processEvents()
            time.sleep(0.03)
        # The window's own pixels, not the screen's: on a busy desktop the
        # compositor may be redrawing the screen while it is read.
        return QGuiApplication.primaryScreen().grabWindow(int(viewport.winId())).toImage()

    def lit() -> int:
        # The desktop may still be fading the window in: grab until two
        # grabs in a row are the same.
        image, previous = grab(), None
        for _ in range(60):
            if image == previous:
                break
            previous, image = image, grab()
        bg = viewport.view.background
        return sum(
            1
            for y in range(0, image.height(), 2)
            for x in range(0, image.width(), 2)
            if max(abs(c - b) for c, b in zip(image.pixelColor(x, y).getRgb()[:3], bg)) > 60
        )

    result["lit"] = lit()
    viewport.view.pan(80.0, 0.0)  # moved, not rebuilt
    result["lit_panned"] = lit()
    result["still_gpu"] = viewport.gpu is not None
    viewport.deleteLater()  # the surface goes before the native window
    for _ in range(5):
        app.processEvents()
    result["closed"] = True
    print(json.dumps(result))
    """
)


def test_a_real_window_is_presented_by_the_gpu(tmp_path: Path) -> None:
    if not os.environ.get("DISPLAY"):
        if REQUIRED:
            pytest.fail("ARQ_GPU_TESTS=require, but there is no X11 display")
        pytest.skip("no X11 display")
    backend = os.environ.get("ARQ_GPU_BACKEND") or "opengl"
    env = dict(os.environ, QT_QPA_PLATFORM="xcb", **{GPU_VIEWPORT_ENV: backend})
    script = tmp_path / "window.py"
    script.write_text(WINDOW_SCRIPT, encoding="utf-8")
    done = subprocess.run(
        [sys.executable, str(script)],
        capture_output=True,
        text=True,
        timeout=120,
        env=env,
        check=False,
    )
    assert done.returncode == 0, done.stderr[-2000:]
    result = json.loads(done.stdout.strip().splitlines()[-1])
    if result["gpu"] is None:
        if REQUIRED:
            pytest.fail(f"ARQ_GPU_TESTS=require, but the GPU did not attach: {done.stderr[-2000:]}")
        pytest.skip(f"no {backend} adapter for a window here")
    assert result["gpu"][0] == backend
    assert result["lit"] > 200, "the drawing is on screen"
    assert result["lit_panned"] > 100, "and still after panning"
    assert result["lit_panned"] != result["lit"], "panning moved it"
    assert result["still_gpu"] and result["closed"]


OVERLAY_SCRIPT = textwrap.dedent(
    """
    import sys, time
    from PySide6.QtCore import QPointF, Qt
    from PySide6.QtGui import QGuiApplication
    from PySide6.QtWidgets import QApplication
    from arqstudiocad.core import CadApplication
    from arqstudiocad.ui.viewport import Viewport

    app = QApplication([])
    document = CadApplication().create_document("Plano")
    first, _ = document.add_line((0.0, 0.0), (100.0, 60.0))
    second, _ = document.add_circle((60.0, 20.0), 15.0)
    document.add_line((0.0, 60.0), (100.0, 0.0))
    viewport = Viewport(document)
    viewport.setWindowFlag(Qt.WindowType.WindowStaysOnTopHint)
    # The real mouse must not move the crosshair or the highlight.
    viewport.setAttribute(Qt.WidgetAttribute.WA_TransparentForMouseEvents)
    viewport.setGeometry(100, 100, 400, 300)
    viewport.show()
    viewport.raise_()
    for _ in range(30):
        app.processEvents()
        time.sleep(0.02)
    # The selection with its grips, one hot; the highlight; a window being
    # dragged; a block dropped; the crosshair.
    viewport.show_selection((first, second))
    grips = tuple(document.grips([first, second]))
    viewport.show_grips(grips, grips[0])
    viewport._set_hover(second)
    viewport._press, viewport._band = QPointF(40, 200), QPointF(260, 260)
    viewport._drop_name = "Door"
    viewport._cursor_at = QPointF(300.0, 120.0)
    viewport.update()

    def grab():
        viewport.repaint()
        for _ in range(3):
            app.processEvents()
            time.sleep(0.03)
        # The window's own pixels, not the screen's: on a busy desktop the
        # compositor may be redrawing the screen while it is read.
        return QGuiApplication.primaryScreen().grabWindow(int(viewport.winId())).toImage()

    # The desktop may still be fading the window in: grab until two grabs
    # in a row are the same.
    shot, previous = grab(), None
    for _ in range(60):
        if shot == previous:
            break
        previous, shot = shot, grab()
    else:
        sys.exit("the window never settled on screen")
    shot.save(sys.argv[1])
    path = "gpu" if viewport.gpu is not None else "cpu"
    print(path, *viewport.view.background)
    viewport.deleteLater()
    for _ in range(5):
        app.processEvents()
    """
)


def _shot(tmp_path: Path, name: str, backend: str | None) -> tuple[str, tuple[int, int, int], Path]:
    script = tmp_path / "overlay.py"
    script.write_text(OVERLAY_SCRIPT, encoding="utf-8")
    env = dict(os.environ, QT_QPA_PLATFORM="xcb")
    env.pop(GPU_VIEWPORT_ENV, None)
    if backend is not None:
        env[GPU_VIEWPORT_ENV] = backend
    out = tmp_path / f"{name}.png"
    done = subprocess.run(
        [sys.executable, str(script), str(out)],
        capture_output=True,
        text=True,
        timeout=120,
        env=env,
        check=False,
    )
    assert done.returncode == 0, done.stderr[-2000:]
    path, *rgb = done.stdout.strip().splitlines()[-1].split()
    r, g, b = (int(c) for c in rgb)
    return path, (r, g, b), out


def test_the_gpu_draws_the_overlays_the_cpu_paints(tmp_path: Path) -> None:
    """One description, two painters (ADR 0034 D4): the selection, the
    highlight, the grips, the window, the drop note and the crosshair look
    the same on both, to within a pixel of antialiasing."""
    if not os.environ.get("DISPLAY"):
        if REQUIRED:
            pytest.fail("ARQ_GPU_TESTS=require, but there is no X11 display")
        pytest.skip("no X11 display")
    backend = os.environ.get("ARQ_GPU_BACKEND") or "opengl"
    path, _, gpu_png = _shot(tmp_path, "gpu", backend)
    if path != "gpu":
        if REQUIRED:
            pytest.fail("ARQ_GPU_TESTS=require, but the GPU did not attach")
        pytest.skip(f"no {backend} adapter for a window here")
    path, (br, bg, bb), cpu_png = _shot(tmp_path, "cpu", None)
    assert path == "cpu"
    gpu, cpu = QImage(str(gpu_png)), QImage(str(cpu_png))
    assert (gpu.width(), gpu.height()) == (cpu.width(), cpu.height())

    def contrast(image: QImage, x: int, y: int) -> int:
        c = image.pixelColor(x, y)
        return max(abs(c.red() - br), abs(c.green() - bg), abs(c.blue() - bb))

    def near(image: QImage, x: int, y: int) -> bool:
        return any(
            contrast(image, x + dx, y + dy) > 0
            for dx in (-1, 0, 1)
            for dy in (-1, 0, 1)
            if 0 <= x + dx < image.width() and 0 <= y + dy < image.height()
        )

    clear = 48
    misses = []
    for y in range(cpu.height()):
        for x in range(cpu.width()):
            for one, other, name in ((cpu, gpu, "the GPU"), (gpu, cpu, "the CPU")):
                if contrast(one, x, y) > clear and not near(other, x, y):
                    misses.append((name, x, y))
    # Measured on the reference machine: none; a missing overlay, hundreds.
    assert not misses, misses[:20]

    def has(image: QImage, rgb: tuple[int, int, int]) -> bool:
        return any(
            _rgb(image.pixelColor(x, y)) == rgb
            for y in range(0, image.height())
            for x in range(0, image.width())
        )

    for color, what in (((30, 144, 255), "selection"), ((230, 40, 40), "hot grip")):
        assert has(cpu, color) and has(gpu, color), what


def _rgb(color: QColor) -> tuple[int, int, int]:
    return (color.red(), color.green(), color.blue())
