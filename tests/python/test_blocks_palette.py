"""The Blocks palette (ADR 0025): the drawing's blocks, and a View-tab
button that toggles the palette and is pressed while it is open (D2)."""

import pytest
from PySide6.QtWidgets import QMenu, QToolButton
from pytestqt.qtbot import QtBot
from ui_fakes import QuietUi

from arqstudiocad.core import CadChangeSet, CadDocument
from arqstudiocad.i18n import set_language
from arqstudiocad.ui.main_window import MainWindow
from arqstudiocad.ui.panel_style import SELECTION_BACKGROUND, SELECTION_LINE
from arqstudiocad.ui.ribbon import RibbonGroupWidget


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


def drawing_with_blocks(window: MainWindow, *names: str) -> CadDocument:
    document = window.session.new_document()
    for index, name in enumerate(names):
        line, _ = document.add_line((index, 0), (index, 1))

        def make(d: CadDocument, n: str = name, i: int = line) -> CadChangeSet:
            return d.create_block(n, (0, 0), [i])[1]

        window.session.edit(document, make)
    return document


def shown(window: MainWindow) -> list[str]:
    rows = window.blocks_panel.list
    return [rows.item(i).text() for i in range(rows.count())]


def palette_button(window: MainWindow) -> QToolButton:
    groups = {g.group.name: g for g in window.findChildren(RibbonGroupWidget)}
    (button,) = groups["palettes"].buttons
    return button


def test_the_palette_lists_the_drawings_blocks_and_filters_them(window: MainWindow) -> None:
    panel = window.blocks_panel
    assert shown(window) == [] and panel.footer.text() == "No drawing is open."
    drawing_with_blocks(window, "Puerta_090", "Mesa", "puerta_080")
    assert shown(window) == ["Mesa", "puerta_080", "Puerta_090"], "as INSERT offers them"
    assert panel.footer.text() == "3 blocks"
    panel.search.setText("PUERTA")
    assert shown(window) == ["puerta_080", "Puerta_090"], "case-insensitive"
    panel.search.setText("mes")
    assert shown(window) == ["Mesa"] and panel.footer.text() == "1 block"
    panel.search.setText("ventana")
    assert shown(window) == [] and panel.footer.text() == "No block matches the search."
    window.session.new_document()
    panel.search.clear()
    assert panel.footer.text() == "The drawing has no blocks."


def test_the_view_tab_button_toggles_the_palette_with_the_layers_focus(
    window: MainWindow,
) -> None:
    panel, action = window.blocks_panel, window.command_actions["BLOCKSPALETTE"]
    button = palette_button(window)
    assert button.defaultAction() is action and button.isCheckable()
    assert SELECTION_BACKGROUND in button.styleSheet() and SELECTION_LINE in button.styleSheet()
    assert action.shortcut().toString() == "Ctrl+3"
    assert panel.isHidden() and not button.isChecked(), "closed on first run, no focus"

    button.click()
    assert window.panel_open(panel) and button.isChecked(), "open: the button has focus"
    view_item = panel.toggleViewAction()
    assert view_item.isChecked(), "the View menu agrees"
    button.click()
    assert panel.isHidden() and not button.isChecked() and not view_item.isChecked()

    view_item.trigger()  # from the View menu
    assert not panel.isHidden() and button.isChecked()
    panel.close()  # from the palette's own close button
    assert not button.isChecked()


def test_the_view_menu_lists_the_palette(window: MainWindow) -> None:
    menus = {m.title(): m for m in window.menuBar().findChildren(QMenu)}
    item = window.blocks_panel.toggleViewAction()
    assert item in menus["&View"].actions() and item.isCheckable()
    assert item.text() == "Blocks"


def test_blockspalette_opens_it_in_english_and_spanish(window: MainWindow) -> None:
    button = palette_button(window)
    window.runner.execute("BLOCKSPALETTE")
    assert not window.blocks_panel.isHidden() and button.isChecked()
    window.close_panel(window.blocks_panel)
    assert not button.isChecked()
    set_language("es")
    try:
        window.runner.execute("PALETABLOQUES")
        assert not window.blocks_panel.isHidden() and button.isChecked()
    finally:
        set_language("en")


def test_an_unpinned_palette_counts_as_open(window: MainWindow) -> None:
    panel, button = window.blocks_panel, palette_button(window)
    button.click()
    assert window.auto_hide.unpin(panel)
    assert button.isChecked(), "collapsed to its side tab, still open"
    button.click()
    assert window.auto_hide.is_pinned(panel) and panel.isHidden() and not button.isChecked()
