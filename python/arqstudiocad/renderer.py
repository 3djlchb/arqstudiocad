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

from arqstudiocad.gpu import GpuProbeResult, backend_order, probe_backend
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
) -> Choice | tuple[str, ...]:
    """The CPU at once, with why, or the backends to probe in order.
    ``window_system`` is what the viewports would present to (``"x11"``,
    ``"win32"``, ``"appkit"``), None where they cannot."""
    if safe_mode:
        return Choice(reason=tr("Drawing on the CPU: safe mode."))
    if setting == CPU:
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
    if window_system not in VALIDATED:
        return Choice(
            reason=tr(
                "Drawing on the CPU: the graphics card is not yet tried on this system. "
                "Choose a renderer in Options to use it."
            ),
            quiet=True,
        )
    return allowed


#: Backends whose probe passes but whose windows stay black, as
#: ``(platform, backend, word in the adapter's name)``: the probe draws
#: offscreen and cannot see it (ADR 0034 D6.4). OpenGL on Intel's Windows
#: driver: the UHD 630 presents black whatever the surface's format, alpha
#: or present mode (2026-11-10); DirectX 12 draws on it.
PRESENTS_BLACK = (("win32", "opengl", "intel"),)


def presents_black(backend: str, adapter: str, platform: str = sys.platform) -> bool:
    """Whether ``backend`` on ``adapter`` is known to leave the window
    black on ``platform`` though its probe passes."""
    name = adapter.lower()
    return any((p, b) == (platform, backend) and word in name for p, b, word in PRESENTS_BLACK)


def probe(
    backends: Sequence[str],
    run: Callable[[str], GpuProbeResult] = probe_backend,
    platform: str = sys.platform,
) -> Choice:
    """The first backend whose probe passes on a hardware adapter that
    draws on screen; the CPU when none does, saying why. Each probe runs in
    a child process."""
    black: tuple[str, str] | None = None
    for backend in backends:
        result = run(backend)
        if not result.available or result.software:
            continue
        adapter = result.adapter or ""
        if presents_black(backend, adapter, platform):
            black = black or (backend, adapter)
            continue
        return Choice(backend=backend, adapter=adapter)
    if black is not None:
        return Choice(
            reason=tr(
                "Drawing on the CPU: {backend} leaves the window black with {adapter} on "
                "this system. Choose another renderer in Options.",
                backend=renderer_title(black[0]),
                adapter=black[1],
            )
        )
    return Choice(reason=tr("Drawing on the CPU: no graphics card passed the check."))


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
