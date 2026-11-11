"""Choosing what draws the viewports (ADR 0034 D5): the GPU where a
hardware adapter passes the probe, the CPU otherwise; the crash sentinel;
the Renderer and Antialiasing settings; the command window told."""

import sys
from collections.abc import Callable
from pathlib import Path
from typing import Any

import pytest
from PySide6.QtCore import QSettings, Qt
from PySide6.QtGui import QGuiApplication
from pytestqt.qtbot import QtBot

from arqstudiocad import renderer
from arqstudiocad.commands import Outcome
from arqstudiocad.gpu import GpuAdapter, GpuProbeResult
from arqstudiocad.renderer import (
    AUTOMATIC,
    CPU,
    CardState,
    Choice,
    Sentinel,
    not_offered,
    plan,
    probe,
    renderer_settings,
    renderer_title,
    survey,
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


def test_the_dialog_offers_the_card_and_its_api_and_says_what_draws(qtbot: QtBot) -> None:
    applied: list[Options] = []
    d = OptionsDialog(Options(), applied.append, renderer_status="Drawing on the CPU: safe mode.")
    qtbot.addWidget(d)
    cards = [d.card.itemData(i) for i in range(d.card.count())]
    assert cards == [AUTOMATIC, CPU], "no card checked yet"
    apis = [d.renderer.itemData(i) for i in range(d.renderer.count())]
    assert apis == [AUTOMATIC, *renderer_settings()[2:]]
    assert d.renderer_status.text() == "Drawing on the CPU: safe mode."
    d.card.setCurrentIndex(d.card.findData(CPU))
    assert not d.renderer.isEnabled(), "the CPU has no graphics API"
    d.antialias.setChecked(True)
    d.apply()
    assert (applied[-1].graphics_card, applied[-1].antialias) == (CPU, True)


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
    monkeypatch.setattr(main_window, "survey", lambda backends: [])
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

    def fake_probe(backends: tuple[str, ...], card: str | None = None) -> Choice:
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
        main_window,
        "probe",
        lambda backends, card=None: Choice(backend="opengl", adapter="Fake GPU"),
    )
    monkeypatch.setattr(main_window, "survey", lambda backends: [])
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


# The graphics card chosen in Options (ADR 0034, "After 02.7")

GTX = "NVIDIA GeForce GTX 1050 Ti"
UHD = "Intel(R) UHD Graphics 630"


def test_every_card_is_surveyed_with_why_it_cannot_draw() -> None:
    def listing(backend: str) -> tuple[list[GpuAdapter], str | None]:
        if backend == "vulkan":
            return [], "crashed"
        return [
            GpuAdapter(backend, 0, UHD, "IntegratedGpu"),
            GpuAdapter(backend, 1, GTX, "DiscreteGpu"),
            GpuAdapter(backend, 2, "Microsoft Basic Render Driver", "Cpu"),
        ], None

    def run(backend: str, index: int | None = None) -> GpuProbeResult:
        if (backend, index) == ("dx12", 0):
            return GpuProbeResult(backend, error="no device")
        return GpuProbeResult(backend, "card", "Other")

    states = survey(("dx12", "vulkan", "opengl"), run, platform="win32", listing=listing)
    found = [(st.backend, st.name, st.usable) for st in states]
    # The discrete cards first; Vulkan listed nothing; no software card.
    assert found == [
        ("dx12", GTX, True),
        ("opengl", GTX, True),
        ("dx12", UHD, False),
        ("opengl", UHD, False),
    ]
    assert "fails its check" in states[2].reason
    assert "leaves the window black" in states[3].reason, "OpenGL on Intel under Windows"


def test_a_card_chosen_is_the_only_one_tried() -> None:
    every = {(b, i) for b in ("dx12", "vulkan") for i in (0, 1)}
    listing, run, ran = two_cards(every)
    chosen = probe(("dx12", "vulkan"), run, platform="win32", listing=listing, card=UHD)
    assert (chosen.backend, chosen.adapter) == ("dx12", UHD)
    assert ran == [("dx12", 0)]
    gone = probe(("dx12",), run, platform="win32", listing=listing, card="Old card")
    assert not gone.gpu and "Old card did not pass the check" in gone.message()


def test_the_cpu_card_draws_on_the_cpu() -> None:
    kw: dict[str, Any] = {"safe_mode": False, "crashed": False, "window_system": "win32"}
    chosen = plan(AUTOMATIC, platform="win32", card=CPU, **kw)
    assert isinstance(chosen, Choice) and not chosen.gpu and chosen.quiet


def test_a_cpu_renderer_kept_before_becomes_the_cpu_card(tmp_path: Path) -> None:
    settings = QSettings(str(tmp_path / "old.ini"), QSettings.Format.IniFormat)
    settings.setValue("options/renderer", CPU)
    kept = saved_options(settings)
    assert (kept.graphics_card, kept.renderer) == (CPU, AUTOMATIC)
    settings.setValue("options/graphics_card", GTX)
    settings.setValue("options/renderer", HERE)
    assert (saved_options(settings).graphics_card, saved_options(settings).renderer) == (GTX, HERE)


def enabled(combo: Any, data: str) -> bool:
    return bool(combo.model().item(combo.findData(data)).isEnabled())


def test_options_lists_the_cards_and_their_apis_with_why_not(qtbot: QtBot) -> None:
    black = "it leaves the window black with this card"
    states = [
        CardState(HERE, GTX, "DiscreteGpu"),
        CardState(HERE, UHD, "IntegratedGpu", reason=black),
    ]
    d = OptionsDialog(Options(), lambda values: None, survey=states)
    qtbot.addWidget(d)
    cards = [d.card.itemData(i) for i in range(d.card.count())]
    assert cards == [AUTOMATIC, CPU, GTX, UHD]
    assert d.card.itemText(2) == f"{GTX} (dedicated)"
    assert d.card.itemText(3) == f"{UHD} (integrated)"
    assert enabled(d.card, GTX) and not enabled(d.card, UHD), "no API draws on the UHD 630"
    d.card.setCurrentIndex(d.card.findData(UHD))
    assert not enabled(d.renderer, HERE)
    assert black in d.cards_status.text()
    d.card.setCurrentIndex(d.card.findData(GTX))
    assert enabled(d.renderer, HERE)
    assert black not in d.cards_status.text()
    others = renderer_settings()[3:]
    assert all(not enabled(d.renderer, api) for api in others), "not offered by this card"
    assert all("does not offer it" in d.cards_status.text() for _ in others)


def test_with_no_card_chosen_a_reason_names_its_card(qtbot: QtBot) -> None:
    black = "it leaves the window black with this card"
    d = OptionsDialog(
        Options(), lambda values: None, survey=[CardState(HERE, UHD, "IntegratedGpu", reason=black)]
    )
    qtbot.addWidget(d)
    assert f"{black} ({UHD})" in d.cards_status.text()


def test_a_card_no_longer_found_is_kept_and_said(qtbot: QtBot) -> None:
    d = OptionsDialog(Options(graphics_card="Old card"), lambda values: None, survey=[])
    qtbot.addWidget(d)
    assert d.card.currentData() == "Old card"
    assert d.card.currentText() == "Old card (not found)"
    assert not enabled(d.card, "Old card")


def test_the_cards_are_checked_again_on_demand(qtbot: QtBot) -> None:
    asked: list[Callable[[Any], None]] = []
    d = OptionsDialog(Options(), lambda values: None, survey=[], check_again=asked.append)
    qtbot.addWidget(d)
    d.check_button.click()
    assert asked and not d.check_button.isEnabled()
    assert d.cards_status.text() == "Checking the graphics cards…"
    asked[0]([CardState(HERE, GTX, "DiscreteGpu")])
    assert d.check_button.isEnabled()
    assert d.card.findData(GTX) >= 0


def test_the_window_surveys_the_cards_and_gives_options_them(
    make_window: Callable[..., MainWindow], monkeypatch: pytest.MonkeyPatch
) -> None:
    states = [CardState(HERE, GTX, "DiscreteGpu")]
    monkeypatch.setattr(main_window, "survey", lambda backends: states)
    window = make_window()
    assert window.options_dialog().card.findData(GTX) < 0, "offscreen: not surveyed at start"
    answered: list[Any] = []
    window.survey_cards(answered.append)
    assert answered == [states]
    assert window.options_dialog().card.findData(GTX) >= 0


def test_the_card_chosen_goes_to_the_probe(
    make_window: Callable[..., MainWindow], monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(QGuiApplication, "platformName", staticmethod(lambda: "xcb"))
    monkeypatch.setattr(main_window, "survey", lambda backends: [])
    asked: list[str | None] = []

    def fake_probe(backends: tuple[str, ...], card: str | None = None) -> Choice:
        asked.append(card)
        return Choice(reason="Drawing on the CPU: no graphics card passed the check.")

    monkeypatch.setattr(main_window, "probe", fake_probe)
    window = make_window()
    window.apply_options(Options(graphics_card=GTX))
    assert asked[-1] == GTX
    window.apply_options(Options(graphics_card=AUTOMATIC))
    assert asked[-1] is None


# Options > Graphics, after the mockup (09-graphics.png)


def test_this_session_says_what_draws_and_what_chose_it(qtbot: QtBot) -> None:
    session = Choice(backend=HERE, adapter=GTX, driver="NVIDIA 527.37")
    d = OptionsDialog(Options(), lambda values: None, session=session, chosen_by="options")
    qtbot.addWidget(d)
    rows = (d.session_card, d.session_driver, d.session_api, d.session_chosen)
    assert [r.text() for r in rows] == [
        GTX,
        "NVIDIA 527.37",
        renderer_title(HERE),
        "chosen in Options",
    ]
    assert not d.renderer_status.isVisibleTo(d), "the rows say what draws"
    cpu = Choice(reason="Drawing on the CPU: safe mode.")
    d = OptionsDialog(
        Options(),
        lambda values: None,
        renderer_status=cpu.message(),
        session=cpu,
        chosen_by="safe_mode",
    )
    qtbot.addWidget(d)
    assert [
        r.text() for r in (d.session_card, d.session_driver, d.session_api, d.session_chosen)
    ] == [
        "CPU (no graphics card)",
        "—",
        "—",
        "safe mode",
    ]
    assert d.renderer_status.text() == "Drawing on the CPU: safe mode.", "and why"


def test_graphics_is_a_section_of_its_own(qtbot: QtBot) -> None:
    d = OptionsDialog(Options(), lambda values: None)
    qtbot.addWidget(d)
    keys = [d.sidebar.item(r).data(Qt.ItemDataRole.UserRole) for r in range(d.sidebar.count())]
    assert keys[-1] == "graphics"
    d.sidebar.setCurrentRow(len(keys) - 1)
    d.card.setCurrentIndex(d.card.findData(CPU))
    d.reset_section()
    assert d.pending.graphics_card == AUTOMATIC, "Reset section resets the card"


def test_the_window_says_what_chose_the_renderer(
    make_window: Callable[..., MainWindow], monkeypatch: pytest.MonkeyPatch
) -> None:
    window = make_window()
    assert window.options_dialog().session_chosen.text() == "automatic"
    window.apply_options(Options(graphics_card=CPU))
    assert window.options_dialog().session_chosen.text() == "chosen in Options"
    window = make_window(safe_mode=True)
    assert window.options_dialog().session_chosen.text() == "safe mode"
    monkeypatch.setenv(GPU_VIEWPORT_ENV, HERE)
    window = make_window()
    assert window.options_dialog().session_chosen.text() == "ARQ_GPU_VIEWPORT"


def test_the_card_s_driver_comes_from_the_probe() -> None:
    result = GpuProbeResult(HERE, GTX, "DiscreteGpu", driver="NVIDIA 527.37")
    run = fake_probes({HERE: result})
    chosen = probe((HERE,), run, platform="win32", listing=unlisted)
    assert chosen.driver == "NVIDIA 527.37"


def test_opengl_on_windows_says_which_card_draws_and_where_to_change_it() -> None:
    why = not_offered("opengl", "win32")
    assert "the card Windows gives" in why and "Display > Graphics" in why
    plain = "this card does not offer it"
    assert not_offered("vulkan", "win32") == plain, "only OpenGL is Windows' choice"
    assert not_offered("opengl", "linux") == plain, "on Linux each card offers its own"


@pytest.mark.skipif(sys.platform != "win32", reason="OpenGL's card is Windows' choice")
def test_options_says_why_opengl_is_not_offered_on_the_dedicated_card(qtbot: QtBot) -> None:
    states = [
        CardState("dx12", GTX, "DiscreteGpu"),
        CardState("dx12", UHD, "IntegratedGpu"),
        CardState("opengl", UHD, "IntegratedGpu"),
    ]
    d = OptionsDialog(Options(), lambda values: None, survey=states)
    qtbot.addWidget(d)
    d.card.setCurrentIndex(d.card.findData(GTX))
    assert not enabled(d.renderer, "opengl")
    assert "OpenGL: " + not_offered("opengl") in d.cards_status.text()
