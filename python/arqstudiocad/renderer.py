"""Which renderer draws the viewports (ADR 0034 D5): the GPU where a
hardware adapter passes the probe, the CPU rasterizer otherwise.

The choice is made in two steps, so starting never waits for the GPU:
:func:`plan` decides at once whether a GPU may be tried and which backends,
from the preference, ``--safe-mode``, the crash sentinel and the platform;
:func:`probe` then tries them, each in a child process (ADR 0007), on a
background thread. A CPU adapter (llvmpipe, lavapipe, WARP) is never taken:
the CPU rasterizer is the fallback before any of them (ADR 0013 D5.8).

The crash sentinel is a file written before the GPU starts drawing and
removed after its first frame. Found at the next start, it means the
graphics driver brought the application down: the GPU is not tried again
until the user picks a renderer in Options.
"""

import contextlib
import sys
from collections.abc import Callable, Sequence
from dataclasses import dataclass
from pathlib import Path

from arqstudiocad.gpu import (
    GpuAdapter,
    GpuProbeResult,
    backend_order,
    list_adapters,
    probe_backend,
)
from arqstudiocad.i18n import tr

#: The Renderer setting: chosen automatically, the CPU, or one backend.
AUTOMATIC = "auto"
CPU = "cpu"


def renderer_settings(platform: str = sys.platform) -> tuple[str, ...]:
    """What the Renderer setting may be on ``platform``: automatic, the CPU,
    and each hardware backend in the order they are tried."""
    return (AUTOMATIC, CPU, *(b for b in backend_order(platform) if b != "software"))


def renderer_title(setting: str) -> str:
    """The setting as the Options dialog names it."""
    titles = {
        AUTOMATIC: tr("Automatic"),
        CPU: tr("CPU (no graphics card)"),
        "dx12": "DirectX 12",
        "vulkan": "Vulkan",
        "opengl": "OpenGL",
        "metal": "Metal",
    }
    return titles.get(setting, setting)


@dataclass(frozen=True)
class Choice:
    """What draws: ``backend`` and ``adapter`` for the GPU, or ``backend``
    None for the CPU, with ``reason`` saying why, in the interface
    language."""

    backend: str | None = None
    adapter: str = ""
    #: The card's index in its backend's list, when it was chosen from it.
    card: int | None = None
    #: The card's driver and its version, as it reports them.
    driver: str = ""
    reason: str = ""
    #: Nothing to tell at start: the CPU where it is expected or chosen.
    quiet: bool = False

    @property
    def gpu(self) -> bool:
        return self.backend is not None

    def message(self) -> str:
        """What the command window says about it."""
        if self.backend is not None and not self.adapter:
            return tr(
                "Drawing with the graphics card ({backend}).",
                backend=renderer_title(self.backend),
            )
        if self.backend is not None:
            return tr(
                "Drawing with the graphics card: {adapter} ({backend}).",
                adapter=self.adapter,
                backend=renderer_title(self.backend),
            )
        return self.reason


#: The window systems where the graphics card has been seen drawing on
#: real hardware (ADR 0034 D6.4): there Automatic uses it. Elsewhere it is
#: used when a backend is chosen in Options, until it is tried.
VALIDATED = frozenset({"x11", "win32"})


def plan(
    setting: str,
    *,
    safe_mode: bool,
    crashed: bool,
    window_system: str | None,
    platform: str = sys.platform,
    card: str = AUTOMATIC,
) -> Choice | tuple[str, ...]:
    """The CPU at once, with why, or the backends to probe in order.
    ``setting`` is the graphics API chosen in Options (or the CPU, as kept
    before the card was chosen apart); ``card`` the graphics card: automatic,
    the CPU, or a card's name. ``window_system`` is what the viewports would
    present to (``"x11"``, ``"win32"``, ``"appkit"``), None where they
    cannot."""
    if safe_mode:
        return Choice(reason=tr("Drawing on the CPU: safe mode."))
    if setting == CPU or card == CPU:
        return Choice(reason=tr("Drawing on the CPU, as chosen in Options."), quiet=True)
    if crashed:
        return Choice(
            reason=tr(
                "Drawing on the CPU: the graphics driver closed {app} the last time it "
                "used the graphics card. Choose a renderer in Options to try it again."
            )
        )
    if window_system is None:
        return Choice(
            reason=tr("Drawing on the CPU: the graphics card cannot draw in this window system."),
            quiet=True,
        )
    allowed = renderer_settings(platform)[2:]
    if setting in allowed:
        return (setting,)
    if window_system not in VALIDATED and card == AUTOMATIC:
        return Choice(
            reason=tr(
                "Drawing on the CPU: the graphics card is not yet tried on this system. "
                "Choose a renderer in Options to use it."
            ),
            quiet=True,
        )
    return allowed


#: Backends whose probe passes, offscreen, but which fail in a window, as
#: ``(platform, backend, word in the adapter's name, how)``: the probe cannot
#: see it (ADR 0034 D6.4). On Intel's Windows driver, tried on the UHD 630
#: (2026-11-10): OpenGL leaves the window black whatever the surface's
#: format, alpha or present mode, and Vulkan closes the application when it
#: draws in a window; DirectX 12 draws.
FAILS_ON_SCREEN = (
    ("win32", "opengl", "intel", "black"),
    ("win32", "vulkan", "intel", "crash"),
)


def fails_on_screen(backend: str, adapter: str, platform: str = sys.platform) -> str | None:
    """How ``backend`` on ``adapter`` is known to fail in a window on
    ``platform`` though its probe passes (``"black"`` or ``"crash"``), or
    None."""
    name = adapter.lower()
    for p, b, word, how in FAILS_ON_SCREEN:
        if (p, b) == (platform, backend) and word in name:
            return how
    return None


def _fails_message(backend: str, adapter: str, how: str) -> str:
    """Why the CPU draws when the only cards left fail in a window."""
    if how == "crash":
        return tr(
            "Drawing on the CPU: {backend} closes {app} when it draws in a window with "
            "{adapter} on this system. Choose another renderer in Options.",
            backend=renderer_title(backend),
            adapter=adapter,
        )
    return tr(
        "Drawing on the CPU: {backend} leaves the window black with {adapter} on "
        "this system. Choose another renderer in Options.",
        backend=renderer_title(backend),
        adapter=adapter,
    )


#: How the graphics cards are tried: the discrete ones first, then the
#: integrated, then the rest (maintainer, 2026-11-10: high performance).
CARD_ORDER = {"DiscreteGpu": 0, "IntegratedGpu": 1}


def probe(
    backends: Sequence[str],
    run: Callable[[str, int | None], GpuProbeResult] = probe_backend,
    platform: str = sys.platform,
    listing: Callable[[str], tuple[list[GpuAdapter], str | None]] = list_adapters,
    card: str | None = None,
) -> Choice:
    """The first graphics card that passes its probe and draws on screen:
    every hardware card of ``backends`` listed, the discrete ones tried
    first, then the integrated, each kind in the backends' order; a backend
    whose cards cannot be listed tries the one it prefers. Only the card
    named ``card`` when one is chosen in Options. The CPU when none passes,
    saying why. Each listing and each probe runs in a child process."""
    tries: list[tuple[int, int, str, int | None, str]] = []
    for order, backend in enumerate(backends):
        adapters, error = listing(backend)
        if error is not None or not adapters:
            if card is None:
                tries.append((len(CARD_ORDER), order, backend, None, ""))
            continue
        tries.extend(
            (CARD_ORDER.get(a.device_type, len(CARD_ORDER)), order, backend, a.index, a.name)
            for a in adapters
            if not a.software and (card is None or a.name == card)
        )
    tries.sort(key=lambda t: (t[0], t[1]))
    failed: tuple[str, str, str] | None = None
    for _, _, backend, index, name in tries:
        how = fails_on_screen(backend, name, platform) if name else None
        if how is not None:
            failed = failed or (backend, name, how)
            continue
        result = run(backend, index)
        if not result.available or result.software:
            continue
        adapter = result.adapter or ""
        how = fails_on_screen(backend, adapter, platform)
        if how is not None:
            failed = failed or (backend, adapter, how)
            continue
        return Choice(backend=backend, adapter=adapter, card=index, driver=result.driver)
    if failed is not None:
        return Choice(reason=_fails_message(*failed))
    if card is not None:
        return Choice(
            reason=tr(
                "Drawing on the CPU: {card} did not pass the check. Choose another graphics "
                "card in Options.",
                card=card,
            )
        )
    return Choice(reason=tr("Drawing on the CPU: no graphics card passed the check."))


@dataclass(frozen=True)
class CardState:
    """A graphics card on one graphics API, as surveyed for Options:
    ``reason`` says why it cannot draw, empty when it can."""

    backend: str
    name: str
    device_type: str
    reason: str = ""

    @property
    def usable(self) -> bool:
        return not self.reason


def not_offered(backend: str, platform: str = sys.platform) -> str:
    """Why a card is not listed on ``backend``, for Options. On Windows,
    OpenGL makes its context on the card Windows gives the application, so
    only that card is listed; the user chooses it in Windows' settings,
    which we leave to them (ADR 0034, "OpenGL on Windows")."""
    if platform == "win32" and backend == "opengl":
        return tr(
            "on Windows it draws only on the card Windows gives {app}; to change that card, "
            "open Windows Settings > System > Display > Graphics"
        )
    return tr("this card does not offer it")


def card_kind(device_type: str) -> str:
    """A card's kind, as Options names it."""
    kinds = {
        "DiscreteGpu": tr("dedicated"),
        "IntegratedGpu": tr("integrated"),
        "VirtualGpu": tr("virtual"),
    }
    return kinds.get(device_type, tr("other"))


def survey(
    backends: Sequence[str],
    run: Callable[[str, int | None], GpuProbeResult] = probe_backend,
    platform: str = sys.platform,
    listing: Callable[[str], tuple[list[GpuAdapter], str | None]] = list_adapters,
) -> list[CardState]:
    """Every hardware card of every one of ``backends``, each probed, for
    Options > Graphics to show which can draw and why the others cannot;
    discrete cards first, as Automatic tries them. A backend whose cards
    cannot be listed adds none. Each listing and probe runs in a child
    process."""
    states: list[tuple[int, int, CardState]] = []
    for order, backend in enumerate(backends):
        adapters, _ = listing(backend)
        for a in adapters:
            if a.software:
                continue
            how = fails_on_screen(backend, a.name, platform)
            if how == "black":
                reason = tr("it leaves the window black with this card")
            elif how == "crash":
                reason = tr("it closes {app} when it draws in a window with this card")
            else:
                result = run(backend, a.index)
                passed = result.available and not result.software
                reason = "" if passed else tr("this card fails its check with it")
            rank = CARD_ORDER.get(a.device_type, len(CARD_ORDER))
            states.append((rank, order, CardState(backend, a.name, a.device_type, reason)))
    states.sort(key=lambda s: (s[0], s[1]))
    return [state for _, _, state in states]


class Sentinel:
    """The crash sentinel: a file present while the GPU is starting."""

    def __init__(self, path: Path) -> None:
        self.path = path

    @property
    def armed(self) -> bool:
        """Whether the last start left it: the driver went down with it."""
        return self.path.exists()

    def arm(self) -> None:
        # Without a place to write it, the GPU still draws.
        with contextlib.suppress(OSError):
            self.path.parent.mkdir(parents=True, exist_ok=True)
            self.path.write_text("the GPU viewport is starting\n", encoding="utf-8")

    def disarm(self) -> None:
        with contextlib.suppress(OSError):
            self.path.unlink(missing_ok=True)
