"""Crash-isolated runtime probes for the 02.7 graphics backends (ADR 0007)."""

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

    @property
    def available(self) -> bool:
        """Whether the offscreen draw and readback succeeded."""
        return self.adapter is not None and self.error is None


def backend_order(platform: str = sys.platform) -> tuple[str, ...]:
    """Returns the backend order from ADR 0007 for this operating system."""
    if platform == "win32":
        return ("dx12", "vulkan", "opengl", "software")
    if platform.startswith("linux"):
        return ("vulkan", "opengl", "software")
    if platform == "darwin":
        return ("metal",)
    return ("software",)


def probe_backend(backend: str, timeout: float = PROBE_TIMEOUT_SECONDS) -> GpuProbeResult:
    """Runs one adapter/pipeline/draw/readback attempt in a child process."""
    if getattr(sys, "frozen", False):
        cli_name = "arqstudiocad-cli.exe" if os.name == "nt" else "arqstudiocad-cli"
        command = [str(Path(sys.executable).with_name(cli_name)), "--gpu-probe", backend]
    else:
        command = [sys.executable, "-m", "arqstudiocad", "--gpu-probe", backend]
    try:
        completed = subprocess.run(
            command,
            capture_output=True,
            check=False,
            text=True,
            timeout=timeout,
        )
    except subprocess.TimeoutExpired:
        return GpuProbeResult(backend, error=f"probe timed out after {timeout:g} seconds")
    except OSError as exc:
        return GpuProbeResult(backend, error=str(exc))

    try:
        payload = json.loads(completed.stdout)
    except json.JSONDecodeError:
        detail = completed.stderr.strip() or "probe process returned invalid output"
        return GpuProbeResult(backend, error=detail)
    if completed.returncode != 0:
        return GpuProbeResult(backend, error=str(payload.get("error", "probe failed")))
    return GpuProbeResult(
        backend=str(payload["backend"]),
        adapter=str(payload["adapter"]),
        device_type=str(payload["device_type"]),
    )


def first_available_backend() -> GpuProbeResult | None:
    """Returns the first passing backend; a crashed or hung probe is skipped."""
    for backend in backend_order():
        result = probe_backend(backend)
        if result.available:
            return result
    return None
