"""Milestone 03.4 reports the integrated core, not the comparison spike."""

import hashlib
import importlib.util
import json
import subprocess
import sys
from pathlib import Path
from types import ModuleType

import pytest
from damaged_files import recovered_dxf, unreadable_dwg

from arqstudiocad.core import CadApplication, LossyConversionError

TOOL = Path(__file__).resolve().parents[2] / "tools" / "compatibility_matrix.py"


@pytest.fixture
def tool() -> ModuleType:
    spec = importlib.util.spec_from_file_location("compatibility_matrix", TOOL)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def drawing(path: Path) -> Path:
    app = CadApplication()
    document = app.create_document("compatibility")
    document.add_line((500_000, 1_000_000), (500_003, 1_000_004))
    document.add_circle((500_010, 1_000_010), 2)
    document.save(path)
    document.close()
    return path


def test_cli_measures_real_roundtrips_without_changing_the_source(tmp_path: Path) -> None:
    source = drawing(tmp_path / "drawing.DWG")
    before = source.read_bytes()
    completed = subprocess.run(
        [sys.executable, str(TOOL), str(source), "--format", "json"],
        capture_output=True,
        text=True,
        check=False,
    )
    assert completed.returncode == 0, completed.stderr
    report = json.loads(completed.stdout)
    (row,) = report["drawings"]
    assert report["schema_version"] == 1 and report["core_version"]
    assert row["read"] == "ok" and row["version"] == "AC1032"
    assert row["sha256"] == hashlib.sha256(before).hexdigest()
    assert row["model_types"] == {"CIRCLE": 1, "LINE": 1}
    assert [(r["format"], r["status"], r["version"]) for r in row["roundtrips"]] == [
        ("dwg", "verified", "AC1032"),
        ("dxf", "verified", "AC1032"),
    ]
    assert source.read_bytes() == before
    assert list(tmp_path.iterdir()) == [source], "saved copies must not pollute the corpus"


def test_empty_or_missing_corpus_cannot_pass(tool: ModuleType, tmp_path: Path) -> None:
    assert tool.main([str(tmp_path)]) == 2
    assert tool.main([str(tmp_path / "missing")]) == 2
    with pytest.raises(ValueError, match="empty"):
        tool.drawings([tmp_path])
    assert not tool.passed([])
    assert not tool.passed([tool.Drawing("unmeasured.dwg", read="ok", roundtrips=[])])


def test_overlapping_roots_do_not_inflate_coverage(tool: ModuleType, tmp_path: Path) -> None:
    source = drawing(tmp_path / "sample.dxf")
    (tmp_path / "notes.txt").write_text("not a drawing", encoding="utf-8")
    assert tool.drawings([tmp_path, source, tmp_path]) == [source.resolve()]
    (tmp_path / "sub").mkdir()
    detour = tmp_path / "sub" / ".."
    assert tool.drawings([detour, source]) == [source.resolve()], "one file, two spellings"


def test_recovery_and_unreadable_files_are_not_clean_reads(
    tool: ModuleType, tmp_path: Path
) -> None:
    recovered = tool.measure(recovered_dxf(tmp_path / "recover.dxf"))
    assert recovered.read == "recovered" and recovered.recovery
    assert [r.status for r in recovered.roundtrips] == ["not_run", "not_run"]
    failed = tool.measure(unreadable_dwg(tmp_path / "bad.dwg"))
    assert failed.read == "error" and failed.error
    assert not tool.passed([recovered, failed])


def test_a_loss_refusal_is_recorded_and_never_accepted(tool: ModuleType, tmp_path: Path) -> None:
    class Refused:
        def save(self, path: Path) -> None:
            error = LossyConversionError("data would be lost")
            error.losses = ["object AEC_TEST: 1 → 0"]
            error.loss_kinds = ["object AEC_TEST"]
            raise error

        def accept_losses(self, kinds: list[str]) -> None:
            pytest.fail("measuring compatibility must not consent to losing data")

    result = tool.roundtrip(CadApplication(), Refused(), tmp_path / "copy.dxf")
    assert result.status == "blocked_loss" and result.losses == ["object AEC_TEST: 1 → 0"]
    row = tool.Drawing("source.dwg", read="ok", roundtrips=[result])
    assert not tool.passed([row])


def test_only_both_formats_verified_pass(tool: ModuleType) -> None:
    def row(dwg: str, dxf: str) -> object:
        trips = [tool.RoundTrip("dwg", dwg), tool.RoundTrip("dxf", dxf)]
        return tool.Drawing("source.dwg", read="ok", roundtrips=trips)

    assert tool.passed([row("verified", "verified")])
    assert not tool.passed([row("verified", "blocked_loss")])
    assert not tool.passed([row("error", "verified")])
    assert not tool.passed([row("verified", "verified"), row("verified", "not_run")])


@pytest.mark.parametrize("mode", ["timeout", "crash", "invalid"])
def test_worker_failures_are_visible(
    tool: ModuleType, tmp_path: Path, monkeypatch: pytest.MonkeyPatch, mode: str
) -> None:
    def run(*args: object, **kwargs: object) -> subprocess.CompletedProcess[str]:
        if mode == "timeout":
            raise subprocess.TimeoutExpired("worker", 1)
        return subprocess.CompletedProcess("worker", 101 if mode == "crash" else 0, "bad JSON", "")

    monkeypatch.setattr(tool.subprocess, "run", run)
    result = tool.isolated(tmp_path / "sample.dwg", 1)
    assert result.read == {"timeout": "timeout", "crash": "crash", "invalid": "error"}[mode]
    assert result.error and not tool.passed([result])


def test_markdown_separates_versions_and_reports_nonadditive_coverage(tool: ModuleType) -> None:
    one = tool.Drawing(
        "a|one.dwg",
        read="ok",
        version="AC1014",
        model_types={"LINE": 2, "CIRCLE": 1},
        roundtrips=[tool.RoundTrip("dwg", "verified"), tool.RoundTrip("dxf", "blocked_loss")],
    )
    two = tool.Drawing("other.dwg", read="error", error="bad\n<file>")
    text = tool.markdown([one, two])
    assert "| DWG | AC1014 | LINE | 1 | 1 | 0 | 1 | 0 |" in text
    assert "| DWG | AC1014 | CIRCLE | 1 | 1 | 0 | 1 | 0 |" in text
    assert "rows are not additive" in text
    assert "a&#124;one.dwg" in text and "bad &lt;file>" in text
    assert "unknown" in text and "not_run" in text


def test_cli_refuses_invalid_timeout_and_overwriting_sources(
    tool: ModuleType, tmp_path: Path
) -> None:
    source = drawing(tmp_path / "input.dwg")
    before = source.read_bytes()
    for value in ("0", "-1", "nan", "inf"):
        assert tool.main([str(source), "--timeout", value]) == 2
    assert tool.main([str(source), "--output", str(source)]) == 2
    alias = tmp_path / "report.md"
    alias.hardlink_to(source)
    assert tool.main([str(source), "--output", str(alias)]) == 2
    assert source.read_bytes() == before
