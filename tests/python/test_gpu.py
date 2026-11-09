"""GPU backend preference and child-process behavior (ADR 0007)."""

import json
import subprocess
import sys
from typing import Any

import pytest

from arqstudiocad import _core, gpu


def test_backend_order_matches_supported_platforms() -> None:
    assert gpu.backend_order("win32") == ("dx12", "vulkan", "opengl", "software")
    assert gpu.backend_order("linux") == ("vulkan", "opengl", "software")
    assert gpu.backend_order("darwin") == ("metal",)


def test_native_probe_rejects_unknown_backend() -> None:
    with pytest.raises(ValueError, match="unsupported GPU backend"):
        _core.probe_gpu_backend("unknown")


def test_probe_runs_native_command_in_a_child(monkeypatch: pytest.MonkeyPatch) -> None:
    calls: list[tuple[list[str], dict[str, object]]] = []

    def run(command: list[str], **kwargs: Any) -> subprocess.CompletedProcess[str]:
        calls.append((command, kwargs))
        return subprocess.CompletedProcess(
            command,
            0,
            json.dumps({"backend": "vulkan", "adapter": "Test GPU", "device_type": "DiscreteGpu"}),
            "",
        )

    monkeypatch.setattr(subprocess, "run", run)
    monkeypatch.setattr(sys, "frozen", False, raising=False)
    result = gpu.probe_backend("vulkan")

    assert result.available
    assert result.adapter == "Test GPU"
    assert calls[0][0][-2:] == ["--gpu-probe", "vulkan"]
    assert calls[0][1]["timeout"] == gpu.PROBE_TIMEOUT_SECONDS


def test_probe_timeout_skips_unresponsive_backend(monkeypatch: pytest.MonkeyPatch) -> None:
    def timeout(*args: object, **kwargs: object) -> subprocess.CompletedProcess[str]:
        raise subprocess.TimeoutExpired("gpu probe", 1)

    monkeypatch.setattr(subprocess, "run", timeout)
    result = gpu.probe_backend("vulkan", timeout=1)

    assert not result.available
    assert result.error == "probe timed out after 1 seconds"


def _fake_probes(
    monkeypatch: pytest.MonkeyPatch, results: dict[str, gpu.GpuProbeResult]
) -> list[str]:
    probed: list[str] = []

    def probe(backend: str) -> gpu.GpuProbeResult:
        probed.append(backend)
        return results.get(backend, gpu.GpuProbeResult(backend, error="not available"))

    monkeypatch.setattr(gpu, "probe_backend", probe)
    monkeypatch.setattr(gpu, "backend_order", lambda: ("vulkan", "opengl", "software"))
    return probed


def test_hardware_adapter_wins_over_an_earlier_software_one(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    # The reference machine: Vulkan is lavapipe, OpenGL ES runs on the GT 730.
    probed = _fake_probes(
        monkeypatch,
        {
            "vulkan": gpu.GpuProbeResult("vulkan", "llvmpipe (LLVM 19.1.7)", "Cpu"),
            "opengl": gpu.GpuProbeResult("opengl", "NV106", "Other"),
            "software": gpu.GpuProbeResult("software", "llvmpipe (LLVM 19.1.7)", "Cpu"),
        },
    )

    result = gpu.first_available_backend()

    assert result == gpu.GpuProbeResult("opengl", "NV106", "Other")
    assert probed == ["vulkan", "opengl"]


def test_first_software_adapter_is_kept_when_no_hardware_passes(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    lavapipe = gpu.GpuProbeResult("vulkan", "llvmpipe (LLVM 19.1.7)", "Cpu")
    probed = _fake_probes(
        monkeypatch,
        {"vulkan": lavapipe, "software": gpu.GpuProbeResult("software", "llvmpipe", "Cpu")},
    )

    assert gpu.first_available_backend() == lavapipe
    assert probed == ["vulkan", "opengl", "software"]


def test_no_backend_when_every_probe_fails(monkeypatch: pytest.MonkeyPatch) -> None:
    _fake_probes(monkeypatch, {})

    assert gpu.first_available_backend() is None
