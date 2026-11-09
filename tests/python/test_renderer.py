"""Choosing what draws the viewports (ADR 0034 D5): the GPU where a
hardware adapter passes the probe, the CPU otherwise; the crash sentinel;
the Renderer and Antialiasing settings; the command window told."""

from collections.abc import Callable
from pathlib import Path
from typing import Any

import pytest
from PySide6.QtCore import QSettings
from PySide6.QtGui import QGuiApplication
from pytestqt.qtbot import QtBot

from arqstudiocad import renderer
from arqstudiocad.commands import Outcome
from arqstudiocad.gpu import GpuProbeResult
from arqstudiocad.renderer import (
    AUTOMATIC,
    CPU,
    Choice,
    Sentinel,
    plan,
    probe,
    renderer_settings,
)
from arqstudiocad.ui import main_window
from arqstudiocad.ui.main_window import MainWindow
from arqstudiocad.ui.options_dialog import Options, OptionsDialog, saved_options
from arqstudiocad.ui.viewport import GPU_VIEWPORT_ENV


def test_the_settings_follow_each_platform_without_software() -> None:
    assert renderer_settings("linux") == (AUTOMATIC, CPU, "vulkan", "opengl")
    assert renderer_settings("win32") == (AUTOMATIC, CPU, "dx12", "vulkan", "opengl")
    assert renderer_settings("darwin") == (AUTOMATIC, CPU, "metal")


def test_the_cpu_is_chosen_at_once_and_says_why() -> None:
    kw: dict[str, Any] = {"crashed": False, "gpu_platform": True, "platform": "linux"}
    for setting, extra, why in (
        (AUTOMATIC, {"safe_mode": True}, "safe mode"),
        (CPU, {"safe_mode": False}, "as chosen in Options"),
        (AUTOMATIC, {"safe_mode": False, "crashed": True}, "Choose a renderer in Options"),
        (AUTOMATIC, {"safe_mode": False, "gpu_platform": False}, "X11"),
    ):
        choice = plan(setting, **{**kw, **extra})
        assert isinstance(choice, Choice) and not choice.gpu
        assert why in choice.message(), (setting, extra)


def test_otherwise_the_backends_to_probe_in_order() -> None:
    kw: dict[str, Any] = {"safe_mode": False, "crashed": False, "gpu_platform": True}
    assert plan(AUTOMATIC, platform="linux", **kw) == ("vulkan", "opengl")
    assert plan("opengl", platform="linux", **kw) == ("opengl",)
    assert plan("metal", platform="linux", **kw) == ("vulkan", "opengl"), "not here"


def fake_probes(results: dict[str, GpuProbeResult]) -> Callable[[str], GpuProbeResult]:
    def run(backend: str) -> GpuProbeResult:
        return results.get(backend, GpuProbeResult(backend, error="not here"))

    return run


def test_the_first_hardware_adapter_is_taken_never_a_software_one() -> None:
    # The reference machine: Vulkan is lavapipe, OpenGL ES the GT 730.
    run = fake_probes(
        {
            "vulkan": GpuProbeResult("vulkan", "llvmpipe (LLVM 19.1.7)", "Cpu"),
            "opengl": GpuProbeResult("opengl", "NV106", "Other"),
        }
    )
    assert probe(("vulkan", "opengl"), run) == Choice(backend="opengl", adapter="NV106")
    only_software = fake_probes({"vulkan": GpuProbeResult("vulkan", "llvmpipe", "Cpu")})
    choice = probe(("vulkan", "opengl"), only_software)
    assert not choice.gpu and "no graphics card passed" in choice.message()


def test_a_choice_says_what_draws() -> None:
    assert Choice(backend="opengl", adapter="NV106").message() == (
        "Drawing with the graphics card: NV106 (OpenGL)."
    )
    assert Choice(backend="vulkan").message() == "Drawing with the graphics card (Vulkan)."


def test_the_sentinel_marks_a_start_until_disarmed(tmp_path: Path) -> None:
    sentinel = Sentinel(tmp_path / "deep" / "gpu-starting")
    assert not sentinel.armed
    sentinel.arm()
    assert sentinel.armed, "a crash now would leave it"
    sentinel.disarm()
    assert not sentinel.armed
    sentinel.disarm()  # twice: no error


def test_renderer_and_antialias_are_kept(tmp_path: Path) -> None:
    settings = QSettings(str(tmp_path / "s.ini"), QSettings.Format.IniFormat)
    assert (saved_options(settings).renderer, saved_options(settings).antialias) == (
        AUTOMATIC,
        False,
    )
    settings.setValue("options/renderer", "opengl")
    settings.setValue("options/antialias", "true")
    assert (saved_options(settings).renderer, saved_options(settings).antialias) == (
        "opengl",
        True,
    )
    settings.setValue("options/renderer", "glide")
    assert saved_options(settings).renderer == AUTOMATIC, "unknown: the default"


def test_the_dialog_offers_the_renderer_and_says_what_draws(qtbot: QtBot) -> None:
    applied: list[Options] = []
    d = OptionsDialog(Options(), applied.append, renderer_status="Drawing on the CPU: safe mode.")
    qtbot.addWidget(d)
    items = [d.renderer.itemData(i) for i in range(d.renderer.count())]
    assert items == list(renderer_settings())
    assert d.renderer_status.text() == "Drawing on the CPU: safe mode."
    d.renderer.setCurrentIndex(d.renderer.findData(CPU))
    d.antialias.setChecked(True)
    d.apply()
    assert (applied[-1].renderer, applied[-1].antialias) == (CPU, True)


class SyncWorker:
    """Runs background work at once, on the calling thread."""

    def run(self, task: Callable[[], Any], done: Callable[[Outcome], None]) -> None:
        try:
            done(Outcome(value=task()))
        except Exception as error:
            done(Outcome(error=error))


def history(window: MainWindow) -> str:
    return window.command_window.history.toPlainText()


@pytest.fixture
def make_window(
    qtbot: QtBot, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> Callable[..., MainWindow]:
    monkeypatch.delenv(GPU_VIEWPORT_ENV, raising=False)

    def make(**kwargs: Any) -> MainWindow:
        settings = QSettings(str(tmp_path / "w.ini"), QSettings.Format.IniFormat)
        window = MainWindow(settings, worker=SyncWorker(), **kwargs)
        qtbot.addWidget(window)
        return window

    return make


def test_offscreen_qt_draws_on_the_cpu(make_window: Callable[..., MainWindow]) -> None:
    window = make_window()
    assert not window.renderer.gpu
    assert "X11" in window.renderer.message(), "Options says why"
    assert history(window) == "", "the CPU where expected: nothing to tell at start"
    window.apply_options(Options(renderer=CPU))
    assert history(window) == "", "nor when chosen"
    assert window.options_dialog().renderer_status.text() == window.renderer.message()


def test_safe_mode_and_a_crash_keep_the_cpu(
    make_window: Callable[..., MainWindow], tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(QGuiApplication, "platformName", staticmethod(lambda: "xcb"))
    window = make_window(safe_mode=True)
    assert not window.renderer.gpu and "safe mode" in history(window)
    sentinel = Sentinel(tmp_path / "gpu-starting")
    sentinel.arm()  # the last start went down
    window = make_window(sentinel=sentinel)
    assert not window.renderer.gpu
    assert "the graphics driver closed" in history(window)
    assert sentinel.armed, "kept until the user chooses again"

    # Choosing a renderer in Options tries the graphics card again.
    probed: list[tuple[str, ...]] = []

    def fake_probe(backends: tuple[str, ...]) -> Choice:
        probed.append(backends)
        return Choice(reason="Drawing on the CPU: no graphics card passed the check.")

    monkeypatch.setattr(main_window, "probe", fake_probe)
    window.apply_options(Options(renderer="opengl"))
    assert not sentinel.armed
    assert probed == [("opengl",)]


def test_a_probe_that_passes_gives_every_viewport_the_gpu_and_a_failure_takes_it_back(
    make_window: Callable[..., MainWindow],
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    qtbot: QtBot,
) -> None:
    # Qt says xcb, the probe passes; offscreen Qt has no X11 window, so the
    # viewport's attach fails: the full path from choice to fallback.
    monkeypatch.setattr(QGuiApplication, "platformName", staticmethod(lambda: "xcb"))
    monkeypatch.setattr(
        main_window, "probe", lambda backends: Choice(backend="opengl", adapter="Fake GPU")
    )
    sentinel = Sentinel(tmp_path / "gpu-starting")
    window = make_window(sentinel=sentinel)
    assert window.renderer == Choice(backend="opengl", adapter="Fake GPU")
    assert "Drawing with the graphics card: Fake GPU (OpenGL)." in history(window)

    document = window.session.new_document()
    viewport = window.viewports[document.id]
    window.show()
    qtbot.waitExposed(window)
    # The attach failed: every viewport back on the CPU, the user told, and
    # no crash left behind.
    assert viewport.gpu is None
    assert not window.renderer.gpu
    assert "stopped drawing" in history(window)
    assert not sentinel.armed


def test_quitting_disarms_the_sentinel(
    make_window: Callable[..., MainWindow], tmp_path: Path
) -> None:
    sentinel = Sentinel(tmp_path / "gpu-starting")
    window = make_window(sentinel=sentinel)
    sentinel.arm()
    window.close()
    assert not sentinel.armed


def test_the_probe_runs_off_the_gui_thread(monkeypatch: pytest.MonkeyPatch) -> None:
    # The window hands the probe to its worker: a probe that would block is
    # never run by plan() itself.
    called: list[str] = []
    monkeypatch.setattr(renderer, "probe_backend", lambda b: called.append(b))
    plan(AUTOMATIC, safe_mode=False, crashed=False, gpu_platform=True, platform="linux")
    assert called == []
