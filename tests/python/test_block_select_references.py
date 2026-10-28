"""SELECTBLOCK (ADR 0025 step 8d, maintainer 2026-10-26): a block's
references in model space are selected, replacing the selection, without
moving the view; those that cannot be selected are counted. The Blocks
palette's Select in drawing starts it with the block chosen."""

from pathlib import Path

import pytest
from pytestqt.qtbot import QtBot
from test_block_editor import LOOKUP
from test_blocks import Harness
from test_blocks_context_menu import actions
from test_blocks_palette import drawing_with_blocks, opened
from test_commands import regression_drawing
from ui_fakes import QuietUi

from arqstudiocad.commands.blocks import select_named
from arqstudiocad.core import CadApplication, InvalidOperationError
from arqstudiocad.i18n import set_language
from arqstudiocad.ui.main_window import MainWindow
from arqstudiocad.ui.viewport import Viewport


def harness() -> tuple[Harness, list[int]]:
    """MESA made from one line and placed three times more; SILLA inside
    a block GRUPO, placed once; a line besides."""
    h = Harness(QuietUi())
    mesa, _ = h.document.create_block("MESA", (10, 10), [h.a])
    placed = [mesa] + [h.document.insert_block("MESA", (x, 0))[0] for x in (20, 30, 40)]
    silla, _ = h.document.create_block("SILLA", (0, 0), [h.b])
    h.document.create_block("GRUPO", (0, 0), [silla, placed[3]])
    h.steps = len(h.document.undo_history)
    return h, placed[:3]


def test_the_api_finds_the_references_and_counts_the_rest() -> None:
    h, placed = harness()
    assert h.document.find_block_references("mesa") == (placed, 0, 1, 0)
    assert h.document.find_block_references("SILLA") == ([], 0, 1, 0)
    with pytest.raises(InvalidOperationError, match="no block named"):
        h.document.find_block_references("NADA")


def test_selectblock_replaces_the_selection_with_the_references() -> None:
    h, placed = harness()
    line, _ = h.document.add_line((0, 0), (1, 1))
    h.steps = len(h.document.undo_history)
    h.session.select(h.document, [line])
    h.run("SELECTBLOCK")
    assert h.prompt().text == "Block name:"
    h.run("nada")
    assert h.messages()[-1] == "There is no such block."
    h.run("mesa")
    assert h.session.selection(h.document) == tuple(placed), "the line is not kept"
    assert h.messages()[-2:] == [
        "3 references to MESA selected.",
        "Not selected: 1 inside other blocks.",
    ]
    assert h.new_steps() == [] and h.runner.active is None, "nothing changed"


def test_the_counts_name_what_cannot_be_selected() -> None:
    h, placed = harness()
    frozen, _ = h.document.create_layer("CONGELADA")
    h.document.set_layer_frozen(frozen, True)
    h.document.set_entity_layer(placed[1:], frozen)
    assert h.document.find_block_references("MESA") == ([placed[0]], 2, 1, 0)
    h.run("SELECTBLOCK", "MESA")
    assert h.session.selection(h.document) == (placed[0],)
    assert h.messages()[-3:] == [
        "1 reference to MESA selected.",
        "Not selected: 2 on frozen or off layers.",
        "Not selected: 1 inside other blocks.",
    ]
    h.run("SELECTBLOCK", "SILLA")
    assert h.session.selection(h.document) == ()
    assert h.messages()[-2:] == [
        "0 references to SILLA selected.",
        "Not selected: 1 inside other blocks.",
    ]


def test_a_dynamic_blocks_references_are_selected() -> None:
    h = Harness(QuietUi())
    document = h.session.open_document(regression_drawing(LOOKUP))
    h.run("SELECTBLOCK", "my_look_block")
    shown, *_ = document.find_block_references("My_Look_Block")
    assert shown and h.session.selection(document) == tuple(shown)
    assert all(e.kind == "INSERT" for e in document.get_entities(shown))


def test_selectblock_in_spanish_and_without_blocks() -> None:
    h = Harness(QuietUi())
    set_language("es")
    h.run("SELECBLOQUE")
    assert h.messages()[-1] == "El dibujo no tiene bloques."
    assert h.runner.active is None


def test_select_named_selects_without_asking() -> None:
    h, placed = harness()
    h.runner.start(select_named("Mesa"))
    assert h.session.selection(h.document) == tuple(placed)
    h.runner.start(select_named("NADA"))
    assert h.messages()[-1] == "There is no such block."


def test_references_in_a_layout_are_counted(tmp_path: Path) -> None:
    codes = [
        *("0", "SECTION", "2", "HEADER", "9", "$ACADVER", "1", "AC1009", "0", "ENDSEC"),
        *("0", "SECTION", "2", "TABLES", "0", "TABLE", "2", "LAYER", "70", "1"),
        *("0", "LAYER", "2", "0", "70", "0", "62", "7", "6", "CONTINUOUS"),
        *("0", "ENDTAB", "0", "ENDSEC", "0", "SECTION", "2", "BLOCKS"),
        *("0", "BLOCK", "8", "0", "2", "MESA", "70", "0", "10", "0", "20", "0", "30", "0"),
        *("3", "MESA", "0", "LINE", "8", "0", "10", "0", "20", "0", "30", "0"),
        *("11", "1", "21", "0", "31", "0", "0", "ENDBLK", "8", "0"),
        *("0", "ENDSEC", "0", "SECTION", "2", "ENTITIES"),
        *("0", "INSERT", "8", "0", "2", "MESA", "10", "0", "20", "0", "30", "0"),
        # In paper space.
        *("0", "INSERT", "8", "0", "67", "1", "2", "MESA", "10", "5", "20", "0", "30", "0"),
        *("0", "ENDSEC", "0", "EOF", ""),
    ]
    path = tmp_path / "layout.dxf"
    path.write_text("\n".join(codes), encoding="ascii")
    h = Harness(QuietUi())
    document = h.session.open_document(path)
    h.run("SELECTBLOCK", "MESA")
    assert len(h.session.selection(document)) == 1
    assert h.messages()[-2:] == [
        "1 reference to MESA selected.",
        "Not selected: 1 in layouts.",
    ]


def test_the_api_on_an_empty_drawing_refuses_any_name() -> None:
    document = CadApplication().create_document("a")
    with pytest.raises(InvalidOperationError):
        document.find_block_references("MESA")


# The Blocks palette's Select in drawing


@pytest.fixture
def window(qtbot: QtBot) -> MainWindow:
    ui = QuietUi()
    w = MainWindow(ui=ui)
    ui.panel_for = w.show_panel
    qtbot.addWidget(w)
    w.resize(1100, 700)
    w.show()
    qtbot.waitExposed(w)
    return w


def test_the_palettes_select_in_drawing_selects_the_references(window: MainWindow) -> None:
    panel = opened(window)
    document = drawing_with_blocks(window, "Mesa", "Silla")
    placed, _ = document.insert_block("Mesa", (5, 5))
    view = window.viewport_for(document)
    assert isinstance(view, Viewport)
    view.view.zoom_at(0, 0, 4.0)  # the references fall off the view
    before = (view.view.scale, view.view.to_world(0, 0))
    panel.select("Mesa")
    found = actions(panel)
    assert found["select"].isEnabled()
    found["select"].trigger()
    (first,) = [i for i in window.session.selection(document) if i != placed]
    assert set(window.session.selection(document)) == {first, placed}
    assert (view.view.scale, view.view.to_world(0, 0)) == before, "the view does not move"
    assert window.runner.active is None
