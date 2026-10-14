"""Failed tests become GitHub annotations in CI (tools/ci_test_annotations.py)."""

import importlib.util
import subprocess
import sys
from pathlib import Path
from types import ModuleType

TOOL = Path(__file__).resolve().parents[2] / "tools" / "ci_test_annotations.py"


def load_tool() -> ModuleType:
    spec = importlib.util.spec_from_file_location("ci_test_annotations", TOOL)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module  # dataclasses look their module up there
    spec.loader.exec_module(module)
    return module


def run_pytest(tmp_path: Path, source: str) -> Path:
    """Runs pytest on a throwaway test file and returns its JUnit report."""
    (tmp_path / "test_sample.py").write_text(source, encoding="utf-8")
    report = tmp_path / "report.xml"
    subprocess.run(
        [
            sys.executable,
            "-m",
            "pytest",
            "-q",
            "-p",
            "no:cacheprovider",
            "test_sample.py",
            f"--junitxml={report}",
        ],
        cwd=tmp_path,
        capture_output=True,
        check=False,
    )
    return report


def annotate(report: Path) -> list[str]:
    done = subprocess.run(
        [sys.executable, str(TOOL), str(report)], capture_output=True, text=True, check=True
    )
    return done.stdout.splitlines()


def test_each_failed_test_becomes_one_annotation(tmp_path: Path) -> None:
    report = run_pytest(
        tmp_path,
        "import pytest\n"
        "def test_fine():\n    assert True\n"
        "def test_broken():\n    assert 1 + 1 == 3, 'arithmetic, 50% off'\n"
        "@pytest.fixture\ndef bad():\n    raise RuntimeError('fixture exploded')\n"
        "def test_errored(bad):\n    pass\n",
    )
    lines = annotate(report)
    assert len(lines) == 2, lines
    broken = next(line for line in lines if "test_broken" in line)
    assert broken.startswith("::error title=test_sample%3A%3Atest_broken::AssertionError")
    assert "arithmetic%2C" not in broken, "the message keeps its commas"
    assert "50%25 off" in broken, "percent signs are escaped"
    errored = next(line for line in lines if "test_errored" in line)
    assert "fixture exploded" in errored


def test_a_passing_run_annotates_nothing(tmp_path: Path) -> None:
    report = run_pytest(tmp_path, "def test_fine():\n    assert True\n")
    assert annotate(report) == []


def test_no_report_means_pytest_did_not_finish(tmp_path: Path) -> None:
    lines = annotate(tmp_path / "missing.xml")
    assert len(lines) == 1 and "did not run or finish" in lines[0]


def test_titles_are_escaped_and_the_count_is_capped() -> None:
    tool = load_tool()
    many = [tool.Failure(f"suite::test_{n}", f"line one\nline {n}") for n in range(13)]
    lines = tool.workflow_commands(many)
    assert len(lines) == tool.LIMIT + 1
    assert lines[0].startswith("::error title=suite%3A%3Atest_0::line one%0Aline 0")
    assert lines[-1] == "::error title=pytest::3 more failed tests"
