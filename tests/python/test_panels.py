"""The layers, properties and history panels (ADR 0013, D5.6)."""

from functools import partial
from pathlib import Path

import pytest
from PySide6.QtCore import Qt
from PySide6.QtWidgets import QTableWidgetItem
from pytestqt.qtbot import QtBot
from ui_fakes import QuietUi

from arqstudiocad.core import CadChangeSet, CadColor, CadDocument
from arqstudiocad.events import WorkFinished, WorkStarted
from arqstudiocad.i18n import set_language
from arqstudiocad.ui import layers_panel
from arqstudiocad.ui.layers_panel import COLOR, CURRENT, FROZEN, NAME, ON, LayersPanel
from arqstudiocad.ui.main_window import MainWindow
from arqstudiocad.ui.properties_panel import version_text


@pytest.fixture
def ui() -> QuietUi:
    return QuietUi()


@pytest.fixture
def window(qtbot: QtBot, ui: QuietUi) -> MainWindow:
    w = MainWindow(ui=ui)
    ui.panel_for = w.show_panel
    qtbot.addWidget(w)
    w.resize(1100, 700)
    w.show()
    qtbot.waitExposed(w)
    return w


def last_line(window: MainWindow) -> str:
    return window.command_window.history.toPlainText().splitlines()[-1]


def cell(panel: LayersPanel, row: int, column: int) -> QTableWidgetItem:
    item = panel.table.item(row, column)
    assert item is not None
    return item


def names(panel: LayersPanel) -> list[str]:
    return [cell(panel, row, NAME).text() for row in range(panel.table.rowCount())]


def row_of(panel: LayersPanel, name: str) -> int:
    return names(panel).index(name)


def checked(panel: LayersPanel, row: int, column: int) -> bool:
    return cell(panel, row, column).checkState() == Qt.CheckState.Checked


def click_check(panel: LayersPanel, row: int, column: int) -> None:
    item = cell(panel, row, column)
    state = item.checkState()
    flipped = Qt.CheckState.Unchecked if state == Qt.CheckState.Checked else Qt.CheckState.Checked
    item.setCheckState(flipped)


# Layers


def test_the_layers_panel_follows_the_active_drawing(window: MainWindow) -> None:
    panel = window.layers_panel
    assert panel.table.rowCount() == 0 and not panel.widget().isEnabled()
    window.runner.execute("NEW")
    assert names(panel) == ["0"] and panel.widget().isEnabled()
    assert cell(panel, 0, CURRENT).text() == "✓"
    first = window.session.active
    assert first is not None
    panel.new_layer()
    window.runner.execute("NEW")
    assert names(panel) == ["0"], "the second drawing has its own layers"
    window.session.activate(first)
    assert names(panel) == ["0", "Layer1"]


def test_new_layers_take_the_first_free_name(window: MainWindow) -> None:
    window.runner.execute("NEW")
    panel = window.layers_panel
    first = panel.new_layer()
    panel.new_layer()
    assert names(panel) == ["0", "Layer1", "Layer2"]
    document = window.session.active
    assert document is not None and document.undo_history == ["LAYER", "LAYER"]
    assert first is not None
    panel._edit(lambda d: d.delete_layer(first))
    panel.new_layer()
    assert sorted(names(panel)) == ["0", "Layer1", "Layer2"]


def test_toggles_change_the_layer(window: MainWindow) -> None:
    window.runner.execute("NEW")
    panel = window.layers_panel
    layer_id = panel.new_layer()
    assert layer_id is not None
    row = row_of(panel, "Layer1")
    click_check(panel, row, ON)
    document = window.session.active
    assert document is not None
    assert not document.get_layer(layer_id).is_visible
    assert not checked(panel, row_of(panel, "Layer1"), ON)
    click_check(panel, row_of(panel, "Layer1"), FROZEN)
    assert document.get_layer(layer_id).is_frozen
    assert document.undo_history == ["LAYER", "LAYER", "LAYER"]


def test_a_refused_change_is_reported_and_undone_in_the_table(window: MainWindow) -> None:
    window.runner.execute("NEW")
    panel = window.layers_panel
    click_check(panel, row_of(panel, "0"), FROZEN)
    document = window.session.active
    assert document is not None and not document.current_layer.is_frozen
    assert not checked(panel, row_of(panel, "0"), FROZEN)
    assert last_line(window).startswith("Error: ")
    assert document.undo_history == []


def test_renaming_in_the_table(window: MainWindow) -> None:
    window.runner.execute("NEW")
    panel = window.layers_panel
    layer_id = panel.new_layer()
    assert layer_id is not None
    cell(panel, row_of(panel, "Layer1"), NAME).setText("A-MUROS")
    document = window.session.active
    assert document is not None and document.get_layer(layer_id).name == "A-MUROS"
    cell(panel, row_of(panel, "A-MUROS"), NAME).setText("0")
    assert last_line(window).startswith("Error: ")
    assert names(panel) == ["0", "A-MUROS"]


def test_setting_current_deleting_and_coloring(window: MainWindow) -> None:
    window.runner.execute("NEW")
    panel = window.layers_panel
    layer_id = panel.new_layer()
    assert layer_id is not None
    panel.table.selectRow(row_of(panel, "Layer1"))
    panel.set_selected_current()
    document = window.session.active
    assert document is not None and document.current_layer.id == layer_id
    assert cell(panel, row_of(panel, "Layer1"), CURRENT).text() == "✓"
    assert cell(panel, row_of(panel, "0"), CURRENT).text() == ""
    panel.set_color(layer_id, CadColor.from_index(1))
    assert document.get_layer(layer_id).color == CadColor.from_index(1)
    assert cell(panel, row_of(panel, "Layer1"), COLOR).text() == "Red"
    panel.table.selectRow(row_of(panel, "Layer1"))
    panel.delete_selected()
    assert last_line(window).startswith("Error: "), "the current layer cannot go"
    panel.table.selectRow(row_of(panel, "0"))
    panel.set_selected_current()
    panel.table.selectRow(row_of(panel, "Layer1"))
    panel.delete_selected()
    assert names(panel) == ["0"]


def test_color_names() -> None:
    assert layers_panel.color_name(CadColor.from_index(5)) == "Blue"
    assert layers_panel.color_name(CadColor.from_index(30)) == "30"
    assert layers_panel.color_name(CadColor.from_rgb(10, 20, 30)) == "10,20,30"


def test_the_layer_command_shows_the_panel(window: MainWindow, ui: QuietUi) -> None:
    window.layers_panel.hide()
    window.runner.execute("NEW")
    window.runner.execute("LA")
    assert ui.panels_shown == ["layers"]
    assert window.layers_panel.isVisible()


def test_show_panel_knows_its_panels(window: MainWindow) -> None:
    window.history_panel.hide()
    assert window.show_panel("history") and window.history_panel.isVisible()
    assert not window.show_panel("toolpalettes")


def test_layers_and_properties_share_tabs_with_history_below(window: MainWindow) -> None:
    assert window.tabifiedDockWidgets(window.layers_panel) == [window.properties_panel]
    assert window.tabifiedDockWidgets(window.history_panel) == []
    area = Qt.DockWidgetArea.RightDockWidgetArea
    assert all(window.dockWidgetArea(panel) == area for panel in window.panels.values())


def test_the_view_menu_toggles_the_panels(window: MainWindow) -> None:
    texts = [action.text() for action in window._view_menu.actions()]
    for title in ("Layers", "Properties", "History", "Command line"):
        assert title in texts


def test_panels_wait_while_background_work_runs(window: MainWindow) -> None:
    window.bus.publish(WorkStarted("Saving…"))
    assert not any(panel.isEnabled() for panel in window.panels.values())
    window.bus.publish(WorkFinished())
    assert all(panel.isEnabled() for panel in window.panels.values())


def test_panels_follow_the_language(window: MainWindow) -> None:
    window.runner.execute("NEW")
    try:
        window.change_language("es")
        assert window.layers_panel.windowTitle() == "Capas"
        header = window.layers_panel.table.horizontalHeaderItem(NAME)
        assert header is not None and header.text() == "Nombre"
        assert window.properties_panel.rows()[0] == ("Nombre", "Drawing1")
        window.layers_panel.new_layer()
        assert "Capa1" in names(window.layers_panel)
    finally:
        set_language("en")


# Properties


def test_properties_describe_the_drawing(window: MainWindow, tmp_path: Path) -> None:
    grid = window.properties_panel.grid
    assert grid.caption.text() == "No drawing open." and window.properties_panel.rows() == []
    window.runner.execute("NEW")
    document = window.session.active
    assert document is not None
    assert grid.caption.text() == "Drawing"
    assert [title for title, _rows in grid.sections()] == ["General", "Contents", "Geometry"]
    rows = dict(window.properties_panel.rows())
    assert rows["Name"] == document.name
    assert rows["File"] == "(not saved)"
    assert rows["Format"] == "AutoCAD 2018 (AC1032)"
    assert rows["Entities"] == "0" and rows["Layers"] == "1"
    assert rows["Current layer"] == "0" and rows["Extents"] == "—"
    assert rows["Unsaved changes"] == "No"
    window.session.edit(document, lambda d: d.add_line((0, 0), (30, 40))[1])
    window.layers_panel.new_layer()
    rows = dict(window.properties_panel.rows())
    assert rows["Entities"] == "1" and rows["Layers"] == "2"
    assert rows["Extents"] == "30.0000 x 40.0000"
    assert rows["Unsaved changes"] == "Yes"


def test_property_sections_fold_and_stay_folded(window: MainWindow) -> None:
    window.runner.execute("NEW")
    grid = window.properties_panel.grid
    contents = grid.tree.topLevelItem(1)
    assert contents is not None and contents.isExpanded()
    assert contents.text(0) == "▾ Contents"
    grid.tree.itemClicked.emit(contents, 0)
    assert not contents.isExpanded() and contents.text(0) == "▸ Contents"
    window.runner.execute("NEW")  # refreshed for another drawing
    again = grid.tree.topLevelItem(1)
    assert again is not None and not again.isExpanded()
    grid.tree.itemClicked.emit(again, 0)
    assert again.isExpanded() and grid.folded == set()


def test_version_text() -> None:
    assert version_text("AC1015") == "AutoCAD 2000 (AC1015)"
    assert version_text("AC9999") == "AC9999"


# History


def history_rows(window: MainWindow) -> list[str]:
    widget = window.history_panel.list
    items = [widget.item(row) for row in range(widget.count())]
    return [item.text() for item in items if item is not None]


def add_three_lines(window: MainWindow, document: CadDocument) -> None:
    for x in (0.0, 10.0, 20.0):
        window.session.edit(document, partial(add_line, start=(x, 0.0), end=(x, 5.0)))


def add_line(
    document: CadDocument, start: tuple[float, float], end: tuple[float, float]
) -> CadChangeSet:
    return document.add_line(start, end)[1]


def test_the_history_lists_every_step(window: MainWindow) -> None:
    assert window.history_panel.list.count() == 0
    window.runner.execute("NEW")
    document = window.session.active
    assert document is not None
    add_three_lines(window, document)
    assert len(history_rows(window)) == 4 and history_rows(window)[0] == "(start)"
    assert window.history_panel.list.currentRow() == 3
    window.runner.execute("UNDO")
    assert window.history_panel.list.currentRow() == 2
    assert len(history_rows(window)) == 4, "the undone step stays, to redo"


def test_choosing_a_row_undoes_and_redoes_to_it(window: MainWindow) -> None:
    window.runner.execute("NEW")
    document = window.session.active
    assert document is not None
    add_three_lines(window, document)
    window.history_panel.list.setCurrentRow(1)
    assert document.entity_count == 1 and len(document.redo_history) == 2
    assert window.history_panel.list.currentRow() == 1
    window.history_panel.list.setCurrentRow(3)
    assert document.entity_count == 3 and not document.can_redo
    window.history_panel.list.setCurrentRow(0)
    assert document.entity_count == 0 and not document.can_undo
    assert not window.viewports[document.id].document.is_modified
