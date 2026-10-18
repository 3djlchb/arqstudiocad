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
