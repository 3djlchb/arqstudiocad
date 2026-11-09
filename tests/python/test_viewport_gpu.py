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
    # Grabbed from the screen: kept above any other window on the desktop.
    viewport.setWindowFlag(Qt.WindowType.WindowStaysOnTopHint)
    viewport.resize(400, 300)
    viewport.show()
    viewport.raise_()
    for _ in range(30):
        app.processEvents()
        time.sleep(0.02)
    result = {"gpu": viewport.gpu}

    def lit() -> int:
        viewport.repaint()
        for _ in range(5):
            app.processEvents()
            time.sleep(0.02)
        top = viewport.mapToGlobal(viewport.rect().topLeft())
        image = QGuiApplication.primaryScreen().grabWindow(
            0, top.x(), top.y(), viewport.width(), viewport.height()
        ).toImage()
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
