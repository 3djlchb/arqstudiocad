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
from collections.abc import Iterator
from pathlib import Path
from typing import Any

import pytest
import shiboken6
from PySide6.QtCore import QEvent, Qt, qInstallMessageHandler
from PySide6.QtGui import QColor, QImage
from PySide6.QtWidgets import QApplication, QVBoxLayout, QWidget
from pytestqt.qtbot import QtBot

from arqstudiocad.core import CadApplication, CadView
from arqstudiocad.ui.viewport import GPU_VIEWPORT_ENV, Viewport

REQUIRED = os.environ.get("ARQ_GPU_TESTS") == "require"


def _need_window_pixels() -> None:
    """Skips, or fails under ``ARQ_GPU_TESTS=require``, where a window's
    pixels cannot be read back from X: without an X server, or in a Wayland
    session, whose XWayland composes nothing and answers with whatever lies
    there. There the composed screen is checked with weston-screenshooter
    instead (ADR 0034 D6.4)."""
    if not os.environ.get("DISPLAY"):
        if REQUIRED:
            pytest.fail("ARQ_GPU_TESTS=require, but there is no X11 display")
        pytest.skip("no X11 display")
    if os.environ.get("WAYLAND_DISPLAY"):
        pytest.skip("a Wayland session: XWayland gives no window's pixels")


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


@pytest.fixture
def qt_messages() -> Iterator[list[str]]:
    """Qt's messages while the test runs."""
    messages: list[str] = []
    previous = qInstallMessageHandler(lambda _kind, _where, text: messages.append(text))
    yield messages
    qInstallMessageHandler(previous)


def test_without_x11_the_viewport_draws_on_the_cpu(
    qtbot: QtBot, monkeypatch: pytest.MonkeyPatch, qt_messages: list[str]
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
    # Qt paints it through its backing store: the widget gives no engine of
    # its own, and never QWidget's, which warns and, on Windows, drops
    # WA_PaintOnScreen when asked while it is being set.
    assert viewport.paintEngine() is None
    assert not [m for m in qt_messages if "paintEngine" in m]
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
    _need_window_pixels()
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
        cwd=tmp_path,
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
        cwd=tmp_path,
    )
    assert done.returncode == 0, done.stderr[-2000:]
    path, *rgb = done.stdout.strip().splitlines()[-1].split()
    r, g, b = (int(c) for c in rgb)
    return path, (r, g, b), out


def test_the_gpu_draws_the_overlays_the_cpu_paints(tmp_path: Path) -> None:
    """One description, two painters (ADR 0034 D4): the selection, the
    highlight, the grips, the window, the drop note and the crosshair look
    the same on both, to within a pixel of antialiasing."""
    _need_window_pixels()
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

    for color, what in (
        ((30, 144, 255), "selection"),
        ((230, 40, 40), "hot grip"),
        ((230, 56, 56), "world X"),
        ((56, 217, 56), "world Y"),
        ((56, 115, 230), "world Z"),
    ):
        assert has(cpu, color) and has(gpu, color), what


def _rgb(color: QColor) -> tuple[int, int, int]:
    return (color.red(), color.green(), color.blue())


APP_SCRIPT = textwrap.dedent(
    """
    import json, sys, time
    from pathlib import Path
    from PySide6.QtCore import QSettings
    from PySide6.QtWidgets import QApplication
    from arqstudiocad.renderer import Sentinel
    from arqstudiocad.ui.main_window import MainWindow

    app = QApplication([])
    folder = Path(sys.argv[1])
    settings = QSettings(str(folder / "settings.ini"), QSettings.Format.IniFormat)
    sentinel = Sentinel(folder / "gpu-starting")
    window = MainWindow(settings, sentinel=sentinel)
    window.resize(800, 600)
    window.show()
    # Unmodified, so closing asks nothing.
    document = window.session.new_document()
    viewport = window.viewports[document.id]
    # The probe runs in the background: wait for its answer and a frame.
    deadline = time.monotonic() + 60
    while time.monotonic() < deadline:
        app.processEvents()
        time.sleep(0.05)
        if viewport.gpu is not None and not sentinel.armed:
            break
    gpu = viewport.gpu
    # How the window goes, sys.argv[2]: closed and the script ended, without
    # an event loop ("close"); closed in the event loop ("exec"); deleted
    # with the viewport inside ("delete"); or after the viewport moved,
    # shown, into another window ("move").
    ending = sys.argv[2]
    moved = None
    if ending == "move" and gpu is not None:
        from PySide6.QtWidgets import QWidget
        other = QWidget()
        other.resize(400, 300)
        other.show()
        viewport.setParent(other)
        viewport.show()
        for _ in range(40):
            app.processEvents()
            time.sleep(0.02)
        moved = viewport.gpu
    print(json.dumps({
        "renderer": [window.renderer.backend, window.renderer.adapter],
        "viewport": gpu,
        "moved": moved,
        "sentinel": sentinel.armed,
        "history": window.command_window.history.toPlainText(),
    }), flush=True)
    if ending == "exec":
        from PySide6.QtCore import QTimer
        QTimer.singleShot(0, window.close)
        app.exec()
    elif ending == "delete":
        import shiboken6
        window.close()
        shiboken6.delete(window)
    else:
        window.close()
    """
)


def _run_app(tmp_path: Path, ending: str) -> dict[str, Any]:
    """APP_SCRIPT's answer, ended as ``ending``: skips without an X11 display
    or a graphics card, and fails under ``ARQ_GPU_TESTS=require``. A crash
    on the way out fails it too."""
    if not os.environ.get("DISPLAY"):
        if REQUIRED:
            pytest.fail("ARQ_GPU_TESTS=require, but there is no X11 display")
        pytest.skip("no X11 display")
    script = tmp_path / "app.py"
    script.write_text(APP_SCRIPT, encoding="utf-8")
    env = dict(os.environ, QT_QPA_PLATFORM="xcb")
    env.pop(GPU_VIEWPORT_ENV, None)
    done = subprocess.run(
        [sys.executable, str(script), str(tmp_path), ending],
        capture_output=True,
        text=True,
        timeout=120,
        env=env,
        check=False,
        cwd=tmp_path,
    )
    assert done.returncode == 0, done.stderr[-2000:]
    result: dict[str, Any] = json.loads(done.stdout.strip().splitlines()[-1])
    if result["renderer"][0] is None:
        if REQUIRED:
            pytest.fail(f"ARQ_GPU_TESTS=require, but the CPU was chosen: {result['history']}")
        pytest.skip(f"no graphics card here: {result['history']}")
    return result


def test_the_app_chooses_the_graphics_card_by_itself(tmp_path: Path) -> None:
    """No backend forced: the window probes, takes a hardware adapter, gives
    the viewport the GPU, says so, and its first frame disarms the
    sentinel (ADR 0034 D5)."""
    result = _run_app(tmp_path, "close")
    backend, adapter = result["renderer"]
    assert result["viewport"] is not None and result["viewport"][0] == backend
    assert result["viewport"][2] != "Cpu", "never a software adapter"
    assert f"Drawing with the graphics card: {adapter}" in result["history"]
    assert not result["sentinel"], "the first frame disarmed it"


@pytest.mark.parametrize("ending", ["exec", "delete", "move"])
def test_the_gpu_lets_go_of_its_window_however_the_window_goes(tmp_path: Path, ending: str) -> None:
    """The application quits cleanly however the viewport's native window
    goes (the plain close is the test above): a swapchain kept past its
    window crashed NVIDIA's driver on the way out. A viewport moved, shown,
    into another window keeps the GPU there (on X11 its native window moves
    with it)."""
    result = _run_app(tmp_path, ending)
    assert result["viewport"] is not None
    if ending == "move":
        assert result["moved"] == result["viewport"], "still presented by the GPU"


def test_the_gpu_is_let_go_before_its_native_window_goes(qtbot: QtBot) -> None:
    """Deleting a window that holds the viewport destroys the viewport's
    native window: the watch on that window lets the GPU go first (ADR
    0034). Offscreen no GPU attaches, so the viewport is told it presents."""
    holder = QWidget()
    document = CadApplication().create_document("Plano")
    viewport = Viewport(document)
    QVBoxLayout(holder).addWidget(viewport)
    viewport.setAttribute(Qt.WidgetAttribute.WA_NativeWindow)
    holder.show()
    qtbot.waitExposed(holder)
    viewport._gpu = True
    viewport._watch_surface()

    shiboken6.delete(holder)

    assert not viewport._gpu, "let go before the window went"


def test_a_new_native_window_takes_the_gpu_only_while_shown(
    qtbot: QtBot, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Once its native window went, the viewport presents into the next one
    it gets while shown; hidden, or closing, it attaches nothing, where
    winId() would make a window for a widget going away."""
    document = CadApplication().create_document("Plano")
    viewport = Viewport(document)
    qtbot.addWidget(viewport)
    viewport.setAttribute(Qt.WidgetAttribute.WA_NativeWindow)
    viewport.show()
    qtbot.waitExposed(viewport)
    attached: list[bool] = []
    monkeypatch.setattr(viewport, "_attach_gpu", lambda: attached.append(True))
    viewport._reattach = True

    QApplication.sendEvent(viewport, QEvent(QEvent.Type.WinIdChange))
    assert attached == [True], "shown: the GPU takes the new window"

    attached.clear()
    viewport.hide()
    QApplication.sendEvent(viewport, QEvent(QEvent.Type.WinIdChange))
    assert attached == [], "hidden: nothing attached"
