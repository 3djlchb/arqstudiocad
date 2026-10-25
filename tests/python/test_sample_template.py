"""The sample template shipped with the application (ADR 0025 D7): ours,
generated, public domain, and readable as a DWG under the `.dwt` name."""

import importlib.util
import sys
from pathlib import Path
from types import ModuleType

import pytest

from arqstudiocad.core import CadApplication
from arqstudiocad.paths import sample_template, templates_dir

TOOL = Path(__file__).resolve().parents[2] / "tools" / "make_sample_template.py"
BLOCKS = [
    "Bed_150",
    "Column_30x30",
    "Door_090",
    "Level_Mark",
    "North_Arrow",
    "Table_4_Chairs",
    "Tree",
    "Washbasin_060",
    "WC",
    "Window_120",
]
DARK = (33, 40, 48)


@pytest.fixture
def tool() -> ModuleType:
    spec = importlib.util.spec_from_file_location("make_sample_template", TOOL)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def drawn(data: bytes | None) -> bool:
    """Whether a preview shows anything."""
    return data is not None and any(data[i + 3] for i in range(0, len(data), 4))


def test_the_sample_template_holds_its_blocks_and_nothing_else() -> None:
    app = CadApplication()
    template = app.open_document(sample_template())
    assert template.recovery_problems == [] and template.undecoded_objects == []
    assert template.get_entities() == [], "a template's model space is empty"
    assert sorted(template.block_names, key=str.lower) == sorted(BLOCKS, key=str.lower)
    for name in BLOCKS:
        assert drawn(template.block_preview(name, 32, DARK)), f"{name} draws"
        info = template.block_info(name)
        assert info is not None and not info.is_dynamic
    colors = {layer.name: layer.color.index for layer in template.get_layers()}
    assert colors["A-DOOR"] == 30 and colors["A-STRUCTURE"] == 1, "coloured layers"


def test_the_shipped_template_is_what_the_script_makes(tool: ModuleType, tmp_path: Path) -> None:
    """Nobody edits the file by hand: it must match a fresh build."""
    fresh = tool.build(tmp_path / "fresh.dwt")
    app = CadApplication()
    shipped, built = app.open_document(sample_template()), app.open_document(fresh)
    assert shipped.block_names == built.block_names
    assert built.get_entities() == [], "the script leaves model space empty"
    for name in BLOCKS:
        assert shipped.block_preview(name, 48, DARK) == built.block_preview(name, 48, DARK), name


def test_the_sample_template_is_public_domain() -> None:
    notice = " ".join((templates_dir() / "README.md").read_text(encoding="utf-8").split())
    assert "CC0 1.0" in notice and "public domain" in notice
    assert "nothing in it comes from another application's templates" in notice


def test_a_template_opens_and_saves_as_a_dwg(tmp_path: Path) -> None:
    app = CadApplication()
    doc = app.create_document("x")
    doc.add_line((0, 0), (1, 1))
    doc.save(tmp_path / "mine.DWT")
    back = app.open_document(tmp_path / "mine.DWT")
    assert back.entity_count == 1 and back.recovery_problems == []
    assert (tmp_path / "mine.DWT").read_bytes()[:4] == b"AC10", "a DWG under another name"
