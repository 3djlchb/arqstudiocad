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
from arqstudiocad.gpu import GpuAdapter, GpuProbeResult
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

#: A hardware backend of the platform running the tests: OpenGL on Linux,
#: DirectX 12 on Windows, Metal on macOS.
HERE = renderer_settings()[2]


def test_the_settings_follow_each_platform_without_software() -> None:
    assert renderer_settings("linux") == (AUTOMATIC, CPU, "vulkan", "opengl")
    assert renderer_settings("win32") == (AUTOMATIC, CPU, "dx12", "vulkan", "opengl")
    assert renderer_settings("darwin") == (AUTOMATIC, CPU, "metal")


def test_the_cpu_is_chosen_at_once_and_says_why() -> None:
    kw: dict[str, Any] = {"crashed": False, "window_system": "x11", "platform": "linux"}
    for setting, extra, why in (
        (AUTOMATIC, {"safe_mode": True}, "safe mode"),
        (CPU, {"safe_mode": False}, "as chosen in Options"),
        (AUTOMATIC, {"safe_mode": False, "crashed": True}, "Choose a renderer in Options"),
        (AUTOMATIC, {"safe_mode": False, "window_system": None}, "this window system"),
    ):
        choice = plan(setting, **{**kw, **extra})
        assert isinstance(choice, Choice) and not choice.gpu
        assert why in choice.message(), (setting, extra)


def test_otherwise_the_backends_to_probe_in_order() -> None:
    kw: dict[str, Any] = {"safe_mode": False, "crashed": False, "window_system": "x11"}
    assert plan(AUTOMATIC, platform="linux", **kw) == ("vulkan", "opengl")
    assert plan("opengl", platform="linux", **kw) == ("opengl",)
    assert plan("metal", platform="linux", **kw) == ("vulkan", "opengl"), "not here"
    # Windows, seen drawing on real hardware (ADR 0034 D6.4): DirectX 12
    # first.
    windows = {**kw, "window_system": "win32", "platform": "win32"}
    assert plan(AUTOMATIC, **windows) == ("dx12", "vulkan", "opengl")


def test_untried_systems_use_the_graphics_card_only_when_chosen() -> None:
    # macOS until seen drawing on real hardware (ADR 0034 D6.4): Automatic
    # keeps the CPU, quietly; a backend chosen in Options is tried.
    kw: dict[str, Any] = {"safe_mode": False, "crashed": False}
    automatic = plan(AUTOMATIC, window_system="appkit", platform="darwin", **kw)
    assert isinstance(automatic, Choice) and not automatic.gpu and automatic.quiet
    assert "Choose a renderer in Options" in automatic.message()
    assert plan("metal", window_system="appkit", platform="darwin", **kw) == ("metal",)


def fake_probes(
    results: dict[str, GpuProbeResult],
) -> Callable[[str, int | None], GpuProbeResult]:
    """Each backend's preferred card, as its probe answers."""

    def run(backend: str, index: int | None = None) -> GpuProbeResult:
        return results.get(backend, GpuProbeResult(backend, error="not here"))

    return run


def unlisted(backend: str) -> tuple[list[GpuAdapter], str | None]:
    """No backend's cards listed: each tries the card it prefers."""
    return [], "not listed"


def test_the_first_hardware_adapter_is_taken_never_a_software_one() -> None:
    # The reference machine: Vulkan is lavapipe, OpenGL ES the GT 730.
    run = fake_probes(
        {
            "vulkan": GpuProbeResult("vulkan", "llvmpipe (LLVM 19.1.7)", "Cpu"),
            "opengl": GpuProbeResult("opengl", "NV106", "Other"),
        }
    )
    assert probe(("vulkan", "opengl"), run, listing=unlisted) == Choice(
        backend="opengl", adapter="NV106"
    )
    only_software = fake_probes({"vulkan": GpuProbeResult("vulkan", "llvmpipe", "Cpu")})
    choice = probe(("vulkan", "opengl"), only_software, listing=unlisted)
    assert not choice.gpu and "no graphics card passed" in choice.message()


def test_a_backend_known_to_draw_black_is_passed_over() -> None:
    # The development laptop: OpenGL's probe passes on the UHD 630, but its
    # window stays black (ADR 0034 D6.4).
    intel = GpuProbeResult("opengl", "Intel(R) UHD Graphics 630", "IntegratedGpu")
    run = fake_probes({"opengl": intel})
    chosen = probe(("opengl",), run, platform="win32", listing=unlisted)
    assert not chosen.gpu
    assert "OpenGL leaves the window black with Intel(R) UHD Graphics 630" in chosen.message()
    # Automatic goes on to the next that draws: DirectX 12, which it tries
    # first anyway.
    both = fake_probes(
        {
            "opengl": intel,
            "dx12": GpuProbeResult("dx12", "Intel(R) UHD Graphics 630", "IntegratedGpu"),
        }
    )
    assert probe(("opengl", "dx12"), both, platform="win32", listing=unlisted).backend == "dx12"
    # OpenGL on another card, or on Intel elsewhere, is taken.
    nvidia = fake_probes(
        {"opengl": GpuProbeResult("opengl", "NVIDIA GeForce GTX 1050 Ti/PCIe/SSE2", "Other")}
    )
    assert probe(("opengl",), nvidia, platform="win32", listing=unlisted).backend == "opengl"
    assert probe(("opengl",), run, platform="linux", listing=unlisted).backend == "opengl"


def test_vulkan_on_intel_under_windows_is_passed_over() -> None:
    # Its probe passes offscreen, but drawing in a window closes the
    # application (an access violation, the development laptop, 2026-11-10).
    intel = GpuProbeResult("vulkan", "Intel(R) UHD Graphics 630", "IntegratedGpu")
    chosen = probe(("vulkan",), fake_probes({"vulkan": intel}), platform="win32", listing=unlisted)
    assert not chosen.gpu
    assert "Vulkan closes" in chosen.message()
    assert "when it draws in a window with Intel(R) UHD Graphics 630" in chosen.message()


def two_cards(
    passing: set[tuple[str, int]],
) -> tuple[
    Callable[[str], tuple[list[GpuAdapter], str | None]],
    Callable[[str, int | None], GpuProbeResult],
    list[tuple[str, int | None]],
]:
    """The development laptop's cards on every backend: the UHD 630 first
    in their own order, the GTX 1050 Ti second; ``passing`` the (backend,
    index) that pass their probe. The probes run are recorded."""
    names = [
        ("Intel(R) UHD Graphics 630", "IntegratedGpu"),
        ("NVIDIA GeForce GTX 1050 Ti", "DiscreteGpu"),
    ]
    ran: list[tuple[str, int | None]] = []

    def listing(backend: str) -> tuple[list[GpuAdapter], str | None]:
        return [GpuAdapter(backend, i, n, kind) for i, (n, kind) in enumerate(names)], None

    def run(backend: str, index: int | None = None) -> GpuProbeResult:
        ran.append((backend, index))
        if index is None or (backend, index) not in passing:
            return GpuProbeResult(backend, error="failed")
        name, kind = names[index]
        return GpuProbeResult(backend, name, kind)

    return listing, run, ran


def test_the_discrete_card_is_tried_first() -> None:
    # Both cards pass everywhere: the GTX on DirectX 12.
    every = {(b, i) for b in ("dx12", "vulkan", "opengl") for i in (0, 1)}
    listing, run, ran = two_cards(every)
    chosen = probe(("dx12", "vulkan", "opengl"), run, platform="win32", listing=listing)
    assert chosen == Choice(backend="dx12", adapter="NVIDIA GeForce GTX 1050 Ti", card=1)
    assert ran == [("dx12", 1)]
    # The GTX failing on DirectX 12: still the GTX, on Vulkan, before the
    # UHD 630 on DirectX 12.
    listing, run, ran = two_cards(every - {("dx12", 1)})
    chosen = probe(("dx12", "vulkan", "opengl"), run, platform="win32", listing=listing)
    assert (chosen.backend, chosen.adapter) == ("vulkan", "NVIDIA GeForce GTX 1050 Ti")
    # The GTX failing everywhere: the integrated card, in the backends'
    # order; OpenGL on it never probed (it draws black on Windows).
    listing, run, ran = two_cards({("dx12", 0), ("opengl", 0)})
    chosen = probe(("opengl", "dx12"), run, platform="win32", listing=listing)
    assert (chosen.backend, chosen.adapter) == ("dx12", "Intel(R) UHD Graphics 630")
    assert ("opengl", 0) not in ran


def test_a_backend_whose_cards_cannot_be_listed_tries_its_own() -> None:
    preferred = GpuProbeResult("vulkan", "NV106", "Other")
    run = fake_probes({"vulkan": preferred})
    chosen = probe(("vulkan",), run, platform="linux", listing=unlisted)
    assert chosen == Choice(backend="vulkan", adapter="NV106")


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
    settings.setValue("options/renderer", HERE)
    settings.setValue("options/antialias", "true")
    assert (saved_options(settings).renderer, saved_options(settings).antialias) == (
        HERE,
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
    assert "this window system" in window.renderer.message(), "Options says why"
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
    window.apply_options(Options(renderer=HERE))
    assert not sentinel.armed
    assert probed == [(HERE,)]


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
    plan(AUTOMATIC, safe_mode=False, crashed=False, window_system="x11", platform="linux")
    assert called == []


def test_a_wayland_session_runs_qt_on_xwayland_for_the_graphics_card() -> None:
    from arqstudiocad.application import qt_platform_for_gpu

    wayland = {"WAYLAND_DISPLAY": "wayland-0", "DISPLAY": ":0"}
    assert qt_platform_for_gpu(wayland, AUTOMATIC, False, "linux") == "xcb"
    assert qt_platform_for_gpu(wayland, None, False, "linux") == "xcb", "nothing saved yet"
    # Qt's own choice stands: the CPU draws, safe mode, no X server, the
    # user chose a platform, or not Linux.
    assert qt_platform_for_gpu(wayland, CPU, False, "linux") is None
    assert qt_platform_for_gpu(wayland, AUTOMATIC, True, "linux") is None
    assert qt_platform_for_gpu({"WAYLAND_DISPLAY": "w"}, AUTOMATIC, False, "linux") is None
    chosen = {**wayland, "QT_QPA_PLATFORM": "wayland"}
    assert qt_platform_for_gpu(chosen, AUTOMATIC, False, "linux") is None
    assert qt_platform_for_gpu(wayland, AUTOMATIC, False, "darwin") is None
    assert qt_platform_for_gpu({"DISPLAY": ":0"}, AUTOMATIC, False, "linux") is None, "X11"
