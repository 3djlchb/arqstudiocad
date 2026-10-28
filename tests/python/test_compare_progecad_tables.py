"""Native table comparisons must reject missing evidence and changed cells."""

from __future__ import annotations

import copy
import importlib.util
import subprocess
import sys
from pathlib import Path
from types import ModuleType

import pytest

TOOL = Path(__file__).resolve().parents[2] / "tools/compare_progecad_tables.py"


@pytest.fixture
def tool() -> ModuleType:
    spec = importlib.util.spec_from_file_location("native_tables", TOOL)
    assert spec and spec.loader
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


@pytest.fixture
def capture(tmp_path: Path) -> Path:
    values = {
        "schema": '"progecad-table-v1"',
        "drawing": '"table.dwg"',
        "table/0/rows": "1",
        "table/0/columns": "1",
        "table/0/height": "14",
        "table/0/horizontal_margin": "1.5",
        "table/0/vertical_margin": "1.5",
        "table/0/style": '"Standard"',
        "table/0/column_width/0": "40",
        "table/0/row_height/0": "14",
        "table/0/text/0/0": '"A label"',
        "table/0/cell_state/0/0": "0",
        "table/0/cell_dxf/0": "((171 . 1) (173 . 0) (175 . 1) (176 . 1) (65 . 1))",
    }
    values.update({f"table/0/text_height/{kind}": "2.5" for kind in (1, 2, 4)})
    path = tmp_path / "capture.tsv"
    path.write_text("".join(f"{k}\t{v}\n" for k, v in values.items()), encoding="utf-8")
    return path


def test_complete_capture_and_numeric_representation(tool: ModuleType, capture: Path) -> None:
    before = tool.observation(capture, "table.dwg", 1)
    after = copy.deepcopy(before)
    after["table/0/height"] += 1e-10
    assert tool.compare(before, after)["matches"]
    assert tool.lisp_value('((175 . 3) (1 . "A label"))') == [[175, 3], [1, "A label"]]


@pytest.mark.parametrize("field", ["horizontal_margin", "text/0/0", "cell_dxf/0"])
def test_margin_text_merge_and_border_changes_are_reported(
    tool: ModuleType, capture: Path, field: str
) -> None:
    before = tool.observation(capture, "table.dwg", 1)
    after = copy.deepcopy(before)
    key = f"table/0/{field}"
    if field == "cell_dxf/0":
        after[key][2][1] = 3.0
        after[key][-1][1] = 2.0
    elif field == "text/0/0":
        after[key] = "Changed"
    else:
        after[key] = 0.06
    report = tool.compare(before, after)
    assert not report["matches"]
    assert [d["key"] for d in report["differences"]] == [key]


@pytest.mark.parametrize("field", ["row_height/0", "cell_dxf/0", "text/0/0"])
def test_incomplete_captures_cannot_pass(tool: ModuleType, capture: Path, field: str) -> None:
    capture.write_text(
        "\n".join(
            line
            for line in capture.read_text().splitlines()
            if not line.startswith(f"table/0/{field}\t")
        ),
        encoding="utf-8",
    )
    with pytest.raises(ValueError):
        tool.observation(capture, "table.dwg", 1)


def test_wrong_drawing_or_missing_table_cannot_pass(tool: ModuleType, capture: Path) -> None:
    with pytest.raises(ValueError, match="different drawing"):
        tool.observation(capture, "other.dwg", 1)
    with pytest.raises(ValueError, match="expected 4 tables"):
        tool.observation(capture, "table.dwg", 4)


@pytest.mark.parametrize("value", ["nan", "inf", "-inf", "(1 . 2", "(1 . 2 3)", "code()"])
def test_nonfinite_and_malformed_native_values_are_rejected(tool: ModuleType, value: str) -> None:
    with pytest.raises(ValueError):
        tool.lisp_value(value)


def test_duplicate_keys_are_rejected(tool: ModuleType, capture: Path) -> None:
    capture.write_text(capture.read_text() + 'schema\t"progecad-table-v1"\n', encoding="utf-8")
    with pytest.raises(ValueError, match="duplicate"):
        tool.observation(capture, "table.dwg", 1)


def test_cli_cannot_overwrite_its_observation(capture: Path) -> None:
    before = capture.read_bytes()
    result = subprocess.run(
        [
            sys.executable,
            str(TOOL),
            str(capture),
            str(capture),
            "--baseline-drawing",
            "table.dwg",
            "--candidate-drawing",
            "table.dwg",
            "--tables",
            "1",
            "--output",
            str(capture),
        ],
        capture_output=True,
        text=True,
        check=False,
    )
    assert result.returncode == 2
    assert capture.read_bytes() == before
