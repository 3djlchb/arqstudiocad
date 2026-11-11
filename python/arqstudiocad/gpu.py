"""Crash-isolated runtime probes for the 02.7 graphics backends (ADR 0007):
each backend's graphics cards listed, and each checked, in a child process."""

import json
import os
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path

PROBE_TIMEOUT_SECONDS = 12


@dataclass(frozen=True)
class GpuProbeResult:
    """Result of one isolated backend attempt."""

    backend: str
    adapter: str | None = None
    device_type: str | None = None
    error: str | None = None
    #: The driver and its version, as the card reports them.
    driver: str = ""

    @property
    def available(self) -> bool:
        """Whether the offscreen draw and readback succeeded."""
        return self.adapter is not None and self.error is None

    @property
    def software(self) -> bool:
        """Whether the adapter runs on the CPU (llvmpipe, lavapipe, WARP)."""
        return self.device_type == "Cpu"


@dataclass(frozen=True)
class GpuAdapter:
    """A graphics card a backend offers, listed before any is checked:
    ``index`` in the backend's own order, what :func:`probe_backend` takes."""

    backend: str
    index: int
    name: str
    device_type: str
    vendor: int = 0
    device: int = 0

    @property
    def software(self) -> bool:
        """Whether it runs on the CPU (llvmpipe, lavapipe, WARP)."""
        return self.device_type == "Cpu"


def backend_order(platform: str = sys.platform) -> tuple[str, ...]:
    """Returns the backend order from ADR 0007 for this operating system."""
    if platform == "win32":
        return ("dx12", "vulkan", "opengl", "software")
    if platform.startswith("linux"):
        return ("vulkan", "opengl", "software")
    if platform == "darwin":
        return ("metal",)
    return ("software",)


def _child(*args: str, timeout: float) -> tuple[dict[str, object] | None, str | None]:
    """Runs this application's command line with ``args`` in a child
    process: its JSON answer, or why there is none."""
    if getattr(sys, "frozen", False):
        cli_name = "arqstudiocad-cli.exe" if os.name == "nt" else "arqstudiocad-cli"
        command = [str(Path(sys.executable).with_name(cli_name)), *args]
    else:
        command = [sys.executable, "-m", "arqstudiocad", *args]
    try:
        completed = subprocess.run(
            command,
            capture_output=True,
            check=False,
            text=True,
            timeout=timeout,
        )
    except subprocess.TimeoutExpired:
        return None, f"probe timed out after {timeout:g} seconds"
    except OSError as exc:
        return None, str(exc)
    try:
        payload = json.loads(completed.stdout)
    except json.JSONDecodeError:
        return None, completed.stderr.strip() or "probe process returned invalid output"
    if completed.returncode != 0:
        return None, str(payload.get("error", "probe failed"))
    return payload, None


def list_adapters(
    backend: str, timeout: float = PROBE_TIMEOUT_SECONDS
) -> tuple[list[GpuAdapter], str | None]:
    """The graphics cards ``backend`` offers, none checked yet, listed in a
    child process; or none, and why."""
    payload, error = _child("--gpu-adapters", backend, timeout=timeout)
    if payload is None:
        return [], error
    listed = payload.get("adapters")
    if not isinstance(listed, list):
        return [], "probe process returned invalid output"
    return [
        GpuAdapter(
            backend=backend,
            index=i,
            name=str(a["name"]),
            device_type=str(a["device_type"]),
            vendor=int(a.get("vendor", 0)),
            device=int(a.get("device", 0)),
        )
        for i, a in enumerate(listed)
    ], None


def probe_backend(
    backend: str, adapter: int | None = None, timeout: float = PROBE_TIMEOUT_SECONDS
) -> GpuProbeResult:
    """Runs one adapter/pipeline/draw/readback attempt in a child process,
    on ``backend``'s ``adapter``-th card, or the one it prefers (the
    discrete card where there are two) when it is None."""
    args = ["--gpu-probe", backend]
    if adapter is not None:
        args += ["--adapter", str(adapter)]
    payload, error = _child(*args, timeout=timeout)
    if payload is None:
        return GpuProbeResult(backend, error=error)
    return GpuProbeResult(
        backend=str(payload["backend"]),
        adapter=str(payload["adapter"]),
        device_type=str(payload["device_type"]),
        driver=str(payload.get("driver", "")),
    )


def first_available_backend() -> GpuProbeResult | None:
    """Returns the first passing hardware backend, else the first software one.

    A crashed or hung probe is skipped. An adapter that runs on the CPU is
    kept only while no hardware one passes (ADR 0013 D5.8): on the reference
    machine Vulkan is lavapipe, so OpenGL ES on the GPU wins over it.
    """
    software: GpuProbeResult | None = None
    for backend in backend_order():
        result = probe_backend(backend)
        if not result.available:
            continue
        if not result.software:
            return result
        if software is None:
            software = result
    return software
