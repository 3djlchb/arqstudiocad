"""The Blocks palette (ADR 0025): the drawing's blocks laid out as the
mockup, the selected block's details, and a View-tab button that toggles
the palette and is pressed while it is open (D2)."""

import json
import os
from pathlib import Path

import pytest
from PySide6.QtCore import QEvent, QMimeData, QPointF, QSettings, QSize, Qt, QUrl
from PySide6.QtGui import (
    QColor,
    QDragEnterEvent,
    QDragLeaveEvent,
    QDragMoveEvent,
    QDropEvent,
    QIcon,
    QImage,
    QPalette,
)
from PySide6.QtWidgets import QApplication, QMenu, QPushButton, QToolButton, QWidget
from pytestqt.qtbot import QtBot
from ui_fakes import QuietUi

from arqstudiocad.commands import ClashChoice
from arqstudiocad.core import CadChangeSet, CadColor, CadDocument
from arqstudiocad.i18n import set_language
from arqstudiocad.paths import sample_template
from arqstudiocad.ui.block_drag import BLOCK_MIME, block_mime, dragged_block
from arqstudiocad.ui.blocks_panel import (
    RECENT_SETTING,
    SESSION_SETTING,
    TAG_BACKGROUND,
    TAG_IN_ROWS,
    TAG_ON_CARDS,
    TAG_STAR,
    TEMPLATES_SETTING,
    BlocksPanel,
    attributes_text,
    references_text,
    template_file,
    units_name,
)
from arqstudiocad.ui.main_window import MainWindow
from arqstudiocad.ui.panel_style import SELECTION_BACKGROUND, SELECTION_LINE
from arqstudiocad.ui.ribbon import RibbonGroupWidget
from arqstudiocad.ui.viewport import Viewport


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
    return window.blocks_panel.shown()


def opened(window: MainWindow) -> BlocksPanel:
    assert window.show_panel("blocks")
    return window.blocks_panel


def palette_button(window: MainWindow, command: str = "BLOCKSPALETTE") -> QToolButton:
    groups = {g.group.name: g for g in window.findChildren(RibbonGroupWidget)}
    action = window.command_actions[command]
    (button,) = [b for b in groups["palettes"].buttons if b.defaultAction() is action]
    return button


def test_the_palette_lists_the_drawings_blocks_and_filters_them(window: MainWindow) -> None:
    panel = opened(window)
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


def test_the_palette_is_laid_out_as_the_mockup(window: MainWindow) -> None:
    panel = opened(window)
    document = drawing_with_blocks(window, "Mesa")
    sources = [(b.text(), b.isEnabled(), b.isChecked()) for b in panel.sources.values()]
    assert sources == [
        ("Template", True, False),
        ("Recent", True, False),
        ("Drawing", True, True),
    ], "the drawing's blocks first"
    assert panel.source_caption.text() == f"Definitions in {document.name}"
    assert panel.grid_view.isChecked() and panel.view() == "grid", "cards first, as the mockup"
    header = panel.table.headerItem()
    assert (header.text(0), header.text(1)) == ("Name", "In drawing")


def test_the_selected_block_shows_its_details_and_references(window: MainWindow) -> None:
    panel = opened(window)
    document = drawing_with_blocks(window, "Mesa", "Silla")
    assert panel.details.sections() == [], "nothing selected"
    assert panel.select("Mesa")
    assert panel.details.sections() == [
        (
            "Selected block",
            [
                ("Name", "Mesa"),
                ("Type", "Static"),
                ("Units", "Unitless"),
                ("Attributes", "No attributes"),
                ("In drawing", "1 reference"),
                ("Source", document.name),
            ],
        )
    ]
    row = panel.table.topLevelItem(0)
    assert row is not None and (row.text(0), row.text(1)) == ("Mesa", "1")
    window.session.edit(document, lambda d: d.insert_block("Mesa", (5, 5))[1])
    assert panel.selected() == "Mesa", "kept across a change"
    assert ("In drawing", "2 references") in panel.details.sections()[0][1]
    panel.search.setText("sil")
    assert panel.selected() is None and panel.details.sections() == []


def test_a_hidden_palette_waits_to_be_shown(window: MainWindow) -> None:
    panel = window.blocks_panel
    drawing_with_blocks(window, "Mesa")
    assert panel.isHidden() and shown(window) == [], "not read while closed"
    opened(window)
    assert shown(window) == ["Mesa"]


def test_the_details_speak_spanish(window: MainWindow) -> None:
    panel = opened(window)
    drawing_with_blocks(window, "Mesa")
    set_language("es")
    try:
        panel.retranslate()
        panel.select("Mesa")
        title, rows = panel.details.sections()[0]
        assert title == "Bloque seleccionado"
        assert ("Atributos", "Ninguno") in rows and ("Tipo", "Estático") in rows
        assert [b.text() for b in panel.sources.values()] == ["Plantilla", "Recientes", "Dibujo"]
    finally:
        set_language("en")


def test_units_attributes_and_references_read_as_the_mockup() -> None:
    assert [units_name(c) for c in (0, 4, 5, 6, 99)] == [
        "Unitless",
        "Millimeters",
        "Centimeters",
        "Meters",
        "Unitless",
    ]
    assert [attributes_text(n) for n in (0, 1, 6)] == [
        "No attributes",
        "1 attribute",
        "6 attributes",
    ]
    assert [references_text(n) for n in (0, 1, 6)] == [
        "Not inserted",
        "1 reference",
        "6 references",
    ]


# Step 2c: insert options and actions (ADR 0025 D5)


def references(document: CadDocument, name: str) -> list[int]:
    return [
        e.id
        for e in document.get_entities()
        if e.kind == "INSERT" and dict(e.details).get("Block name") == name
    ]


def box(document: CadDocument, entity_id: int) -> tuple[float, float, float, float]:
    bounds = document.get_entity(entity_id).bounds
    assert bounds is not None
    return (
        round(bounds.min[0], 6),
        round(bounds.min[1], 6),
        round(bounds.max[0], 6),
        round(bounds.max[1], 6),
    )


def test_the_actions_follow_the_drawing_and_the_selection(window: MainWindow) -> None:
    panel = opened(window)
    buttons = (panel.insert_button, panel.edit_button, panel.new_button)
    assert [b.isEnabled() for b in buttons] == [False, False, False], "no drawing"
    drawing_with_blocks(window, "Mesa")
    assert [b.isEnabled() for b in buttons] == [False, False, True], "nothing selected"
    panel.select("Mesa")
    assert [b.isEnabled() for b in buttons] == [True, True, True]
    assert (panel.insert_button.text(), panel.edit_button.text(), panel.new_button.text()) == (
        "Insert",
        "Edit block",
        "New…",
    )


def test_insert_at_the_origin_with_the_options_is_one_step(window: MainWindow) -> None:
    panel = opened(window)
    document = drawing_with_blocks(window, "Mesa")  # a line (0,0)-(0,1)
    before = references(document, "Mesa")
    steps = len(document.undo_history)
    panel.select("Mesa")
    panel.scale.setValue(2.0)
    panel.rotation.setValue(90.0)
    panel.on_screen.setChecked(False)
    panel.insert_button.click()
    (new,) = set(references(document, "Mesa")) - set(before)
    assert box(document, new) == (-2.0, 0.0, 0.0, 0.0), "twice as big, turned 90°"
    assert len(document.undo_history) == steps + 1 and document.undo_name == "INSERT"
    assert "Inserting block Mesa." in window.command_window.history.toPlainText()
    assert window.runner.active is None
    assert ("In drawing", "2 references") in panel.details.sections()[0][1]
    document.undo()
    assert references(document, "Mesa") == before


def test_insert_on_screen_follows_the_cursor_until_a_point(window: MainWindow) -> None:
    panel = opened(window)
    document = drawing_with_blocks(window, "Mesa")
    before = references(document, "Mesa")
    panel.select("Mesa")
    panel.table.itemActivated.emit(panel.table.currentItem(), 0)  # Enter or double click
    assert window.runner.active == "INSERT"
    prompt = window.runner.prompt
    assert prompt is not None and prompt.message == "Specify insertion point"
    window.runner.answer_point((10.0, 5.0))
    (new,) = set(references(document, "Mesa")) - set(before)
    assert box(document, new) == (10.0, 5.0, 10.0, 6.0)
    assert window.runner.active is None and document.undo_name == "INSERT"


def test_edit_and_new_start_bedit_and_block(window: MainWindow) -> None:
    panel = opened(window)
    document = drawing_with_blocks(window, "Mesa")
    panel.select("Mesa")
    panel.new_button.click()
    assert window.runner.active == "BLOCK"
    window.runner.cancel()
    panel.edit_button.click()
    editor = window.session.active
    assert editor is not None and window.session.block_editor_of(editor) == (document, "Mesa")


def test_the_insert_options_fold_and_speak_spanish(window: MainWindow) -> None:
    panel = opened(window)
    assert panel.options.isVisible() and panel.on_screen.isChecked()
    assert (panel.scale.value(), panel.rotation.value()) == (1.0, 0.0)
    panel.options_band.click()
    assert not panel.options.isVisible()
    panel.options_band.click()
    assert panel.options.isVisible()
    set_language("es")
    try:
        panel.retranslate()
        texts = (panel.options_band.text(), panel.on_screen.text(), panel.insert_button.text())
        assert texts == ("Opciones de inserción", "Precisar en pantalla", "Insertar")
    finally:
        set_language("en")


# Step 2d: Properties and Layers join the Palettes group (ADR 0025 D2)


def test_the_palettes_group_holds_blocks_layers_and_properties(window: MainWindow) -> None:
    groups = {g.group.name: g for g in window.findChildren(RibbonGroupWidget)}
    shown = [b.defaultAction() for b in groups["palettes"].buttons]
    actions = window.command_actions
    assert shown == [actions["BLOCKSPALETTE"], actions["LAYER"], actions["PROPERTIES"]]
    assert actions["PROPERTIES"].shortcut().toString() == "Ctrl+1"
    assert actions["PROPERTIES"].text() == "&Properties"
    for command in ("LAYER", "PROPERTIES"):
        assert palette_button(window, command).isChecked(), f"{command}: open at start"


def test_the_properties_button_toggles_its_panel(window: MainWindow) -> None:
    panel, button = window.properties_panel, palette_button(window, "PROPERTIES")
    button.click()
    assert panel.isHidden() and not button.isChecked()
    button.click()
    assert not panel.isHidden() and button.isChecked()
    window.close_panel(panel)
    for line in ("PROPERTIES", "PR"):
        window.runner.execute(line)
        assert not panel.isHidden() and button.isChecked(), f"{line}, without a drawing"
        window.close_panel(panel)
    set_language("es")
    try:
        window.runner.execute("PROPIEDADES")
        assert not panel.isHidden() and button.isChecked()
    finally:
        set_language("en")


def test_a_refused_command_leaves_its_button_up(window: MainWindow) -> None:
    """LAYER needs a drawing: pressed without one, the button does not stay down."""
    panel, button = window.layers_panel, palette_button(window, "LAYER")
    button.click()
    assert panel.isHidden() and not button.isChecked()
    button.click()  # no drawing: LAYER refuses
    assert panel.isHidden() and not button.isChecked()
    window.session.new_document()
    button.click()
    assert not panel.isHidden() and button.isChecked()


# Step 3: previews and the grid view (ADR 0025 D3)


def test_the_core_names_changed_blocks_and_draws_their_previews() -> None:
    from arqstudiocad.core import CadApplication

    document = CadApplication().create_document("previews")
    line, _ = document.add_line((0, 0), (10, 10))
    _, made = document.create_block("Mesa", (0, 0), [line])
    assert made.blocks_created == ("Mesa",) and made.blocks_deleted == ()
    undone = document.undo()
    assert undone.blocks_deleted == ("Mesa",)
    document.redo()
    data = document.block_preview("mesa", 32, (0, 0, 0), ink=(255, 0, 0))
    assert data is not None and len(data) == 32 * 32 * 4
    pixels = [data[i : i + 4] for i in range(0, len(data), 4)]
    assert any(px[3] for px in pixels) and pixels[0][3] == 0, "ink on transparency"
    assert document.block_preview("Mesa", 32, (0, 0, 0)) is not None, "its own colors"
    assert document.block_preview("nada", 32, (0, 0, 0)) is None
    with pytest.raises(ValueError, match="1 to 1024"):
        document.block_preview("Mesa", 0, (0, 0, 0))


def cards(panel: BlocksPanel) -> list[str]:
    return [panel.grid.item(i).text() for i in range(panel.grid.count())]


def test_the_grid_shows_cards_with_previews_in_their_layers_colors(window: MainWindow) -> None:
    panel = opened(window)
    document = drawing_with_blocks(window, "Silla", "Puerta")
    red, _ = document.create_layer("Rojo")
    document.set_layer_color(red, CadColor.from_index(1))
    line, _ = document.add_line((0, 0), (4, 3))
    document.set_entity_layer([line], red)
    window.session.edit(document, lambda d: d.create_block("Mesa", (0, 0), [line])[1])
    assert panel.views.currentWidget() is panel.grid
    assert cards(panel) == ["Mesa", "Puerta", "Silla"] == shown(window)
    size = QSize(panel.grid.card_size(), panel.grid.card_size())

    def ink(name: str, mode: QIcon.Mode = QIcon.Mode.Normal) -> QColor:
        """The color of the preview's most opaque pixel: its stroke's."""
        card = panel.grid.item(cards(panel).index(name))
        image = card.icon().pixmap(size, mode).toImage()
        pixels = [
            image.pixelColor(x, y) for x in range(image.width()) for y in range(image.height())
        ]
        return max(pixels, key=lambda c: c.alpha())

    def near(color: QColor, rgb: tuple[int, int, int]) -> bool:
        channels = (color.red(), color.green(), color.blue())
        return max(abs(a - b) for a, b in zip(channels, rgb, strict=True)) <= 6

    assert near(ink("Mesa"), (255, 0, 0)), "the red layer's color"
    dark = panel.palette().color(QPalette.ColorRole.Base).lightness() < 128
    assert near(ink("Silla"), (255, 255, 255) if dark else (0, 0, 0)), "ACI 7 on the theme"
    assert ink("Mesa", QIcon.Mode.Selected) == ink("Mesa"), "the card's frame shows selection"
    row = panel.table.topLevelItem(0)
    assert row is not None and not row.icon(0).isNull(), "a small preview in the list"


def test_a_card_and_a_row_are_one_selection(window: MainWindow) -> None:
    panel = opened(window)
    drawing_with_blocks(window, "Mesa", "Silla")
    panel.grid.setCurrentItem(panel.grid.item(1))
    assert panel.selected() == "Silla"
    assert panel.details.sections()[0][1][0] == ("Name", "Silla")
    panel.select("Mesa")
    current = panel.grid.currentItem()
    assert current is not None and current.text() == "Mesa"
    panel.grid.itemActivated.emit(current)  # Enter or a double click on a card
    assert window.runner.active == "INSERT"


def test_the_chosen_view_is_remembered(qtbot: QtBot, tmp_path: Path) -> None:
    settings = QSettings(str(tmp_path / "settings.ini"), QSettings.Format.IniFormat)
    first = MainWindow(ui=QuietUi(), settings=settings)
    qtbot.addWidget(first)
    first.blocks_panel.list_view.click()
    assert first.blocks_panel.views.currentWidget() is first.blocks_panel.table
    second = MainWindow(ui=QuietUi(), settings=settings)
    qtbot.addWidget(second)
    assert second.blocks_panel.view() == "list"


def test_previews_are_drawn_again_only_when_a_block_changes(window: MainWindow) -> None:
    panel = opened(window)
    document = drawing_with_blocks(window, "Mesa", "Silla")
    drawn = panel.previews.drawn
    window.session.edit(document, lambda d: d.add_line((50, 50), (60, 60))[1])
    window.session.edit(document, lambda d: d.insert_block("Mesa", (5, 5))[1])
    assert panel.previews.drawn == drawn, "the blocks did not change"
    line, _ = document.add_line((0, 0), (0, 3))
    window.session.edit(document, lambda d: d.create_block("Puerta", (0, 0), [line])[1])
    assert panel.previews.drawn > drawn, "a block changed: every preview again"
    again = panel.previews.drawn
    window.session.edit(document, lambda d: d.undo())
    assert panel.previews.drawn > again and "Puerta" not in cards(panel), "undo too"


# Step 4: the dynamic-block tag (ADR 0025 D4)


def dynamic_drawing(window: MainWindow) -> CadDocument:
    """AcadRust's lookup sample: one dynamic block, My_Look_Block."""
    from test_block_editor import LOOKUP
    from test_commands import regression_drawing

    document = window.session.open_document(regression_drawing(LOOKUP))
    window.session.activate(document)
    return document


def tagged(image: QImage) -> bool:
    """Whether the tag sits in the preview's top-left corner: orange at its
    edge, the turquoise star in its middle."""
    ratio = image.devicePixelRatio()
    width = image.width() / ratio
    # Rows' previews are 24 px with the small tag; the legend is the tag alone.
    side = (TAG_IN_ROWS if TAG_ON_CARDS < width <= 24 else TAG_ON_CARDS) * ratio
    edge = image.pixelColor(round(ratio), round(side / 2))
    middle = image.pixelColor(round(side / 2), round(side / 2))
    return (edge.name(), middle.name()) == (TAG_BACKGROUND, TAG_STAR)


def test_a_dynamic_block_carries_the_tag_and_cannot_be_edited(window: MainWindow) -> None:
    assert (TAG_BACKGROUND, TAG_STAR) == ("#ffa64d", "#004b4e"), "the maintainer's colors"
    panel = opened(window)
    document = dynamic_drawing(window)
    line, _ = document.add_line((0, 0), (5, 5))
    window.session.edit(document, lambda d: d.create_block("Estatico", (0, 0), [line])[1])
    names = cards(panel)
    dynamic, static = names.index("My_Look_Block"), names.index("Estatico")
    size = QSize(panel.grid.card_size(), panel.grid.card_size())
    for mode in (QIcon.Mode.Normal, QIcon.Mode.Selected):
        assert tagged(panel.grid.item(dynamic).icon().pixmap(size, mode).toImage()), mode
        assert not tagged(panel.grid.item(static).icon().pixmap(size, mode).toImage())
    row = panel.table.topLevelItem(names.index("My_Look_Block"))
    assert row is not None
    assert tagged(row.icon(0).pixmap(QSize(24, 24)).toImage()), "in the list too"
    card = panel.grid.item(dynamic)
    assert card.toolTip() == "My_Look_Block\nDynamic block"
    assert card.data(Qt.ItemDataRole.AccessibleDescriptionRole) == "Dynamic block"
    assert panel.legend.isVisible() and panel.legend_text.text() == "Dynamic block"
    assert tagged(panel.legend_tag.pixmap().toImage()), "the legend shows the tag itself"

    panel.select("My_Look_Block")
    assert not panel.edit_button.isEnabled() and panel.insert_button.isEnabled()
    assert panel.edit_button.toolTip() == (
        "My_Look_Block is a dynamic block: it cannot be edited yet."
    )
    info = document.block_info("My_Look_Block")
    assert info is not None and info.is_dynamic and info.parameters
    rows = dict(panel.details.sections()[0][1])
    assert rows["Type"] == "Dynamic block · " + ", ".join(info.parameters)
    panel.select("Estatico")
    assert panel.edit_button.isEnabled() and panel.edit_button.toolTip() == ""
    panel.search.setText("estat")
    assert not panel.legend.isVisible(), "no dynamic block shown: no legend"


def test_the_tag_is_a_symbol_the_same_in_every_language(window: MainWindow) -> None:
    panel = opened(window)
    dynamic_drawing(window)
    index = cards(panel).index("My_Look_Block")
    size = QSize(panel.grid.card_size(), panel.grid.card_size())
    english = panel.grid.item(index).icon().pixmap(size).toImage()
    drawn = panel.previews.drawn
    set_language("es")
    try:
        panel.retranslate()
        assert panel.previews.drawn == drawn, "no tag to draw again"
        spanish = panel.grid.item(index).icon().pixmap(size).toImage()
        assert spanish == english and tagged(spanish)
        assert panel.legend_text.text() == "Bloque dinámico"
        card = panel.grid.item(index)
        assert card.toolTip() == "My_Look_Block\nBloque dinámico"
    finally:
        set_language("en")


def test_the_grid_scrolls_to_the_selected_card(qtbot: QtBot, window: MainWindow) -> None:
    panel = opened(window)
    drawing_with_blocks(window, *(f"Bloque_{i:02}" for i in range(30)))
    last = panel.grid.item(panel.grid.count() - 1)
    assert not panel.grid.viewport().rect().intersects(panel.grid.visualItemRect(last))
    panel.select("Bloque_29")
    qtbot.waitUntil(
        lambda: panel.grid.viewport().rect().intersects(panel.grid.visualItemRect(last))
    )


def first_row(panel: BlocksPanel) -> int:
    """How many cards the grid has laid out in its first row."""
    top = panel.grid.visualItemRect(panel.grid.item(0)).top()
    rects = (panel.grid.visualItemRect(panel.grid.item(i)) for i in range(panel.grid.count()))
    return sum(1 for rect in rects if rect.top() == top)


def test_a_wider_palette_holds_more_columns_of_the_same_cards(
    qtbot: QtBot, window: MainWindow
) -> None:
    panel = opened(window)
    drawing_with_blocks(window, *(f"Bloque_{i:02}" for i in range(12)))
    panel.setFloating(True)
    panel.resize(340, 700)  # about the narrowest it goes
    qtbot.wait(50)
    narrow = first_row(panel)
    assert 1 < narrow < 12, "the cards wrap into rows"
    drawn = panel.previews.drawn
    panel.resize(900, 700)
    qtbot.waitUntil(lambda: first_row(panel) > narrow)
    assert panel.grid.iconSize() == QSize(64, 64), "the previews keep their size"
    assert panel.previews.drawn == drawn, "nothing drawn again"


# Step 5: drag and drop onto the drawing, and Explode (ADR 0025 D5)


def drag_event(kind: str, viewport: QWidget, mime: QMimeData, at: QPointF) -> QEvent:
    """A Qt drag event over ``viewport`` at ``at`` (logical pixels)."""
    actions = Qt.DropAction.CopyAction
    buttons, modifiers = Qt.MouseButton.LeftButton, Qt.KeyboardModifier.NoModifier
    if kind == "enter":
        return QDragEnterEvent(at.toPoint(), actions, mime, buttons, modifiers)
    if kind == "move":
        return QDragMoveEvent(at.toPoint(), actions, mime, buttons, modifiers)
    return QDropEvent(at, actions, mime, buttons, modifiers)


def screen(viewport: Viewport, x: float, y: float) -> QPointF:
    sx, sy = viewport.view.to_screen(x, y)
    ratio = viewport.devicePixelRatioF()
    return QPointF(sx / ratio, sy / ratio)


def test_a_dragged_block_carries_its_drawing_and_name(window: MainWindow) -> None:
    panel = opened(window)
    document = drawing_with_blocks(window, "Mesa")
    card, row = panel.grid.item(0), panel.table.topLevelItem(0)
    assert row is not None
    for mime in (panel.grid.mimeData([card]), panel.table.mimeData([row])):
        assert dragged_block(mime) == (document.id, "Mesa")
    assert dragged_block(QMimeData()) is None, "not a block"
    broken = QMimeData()
    broken.setData(BLOCK_MIME, b"{not json")
    assert dragged_block(broken) is None


def test_dropping_a_block_inserts_it_where_it_lands(window: MainWindow) -> None:
    opened(window)
    document = drawing_with_blocks(window, "Mesa")  # a line (0,0)-(0,1)
    viewport = window.viewports[document.id]
    viewport.zoom_extents()
    before = references(document, "Mesa")
    steps = len(document.undo_history)
    mime = block_mime(document.id, "Mesa")
    QApplication.sendEvent(viewport, drag_event("enter", viewport, mime, screen(viewport, 0, 0)))
    assert window.runner.active == "INSERT", "the block follows the cursor"
    assert viewport.target is not None
    passing = screen(viewport, 0.1, 0.1)
    QApplication.sendEvent(viewport, drag_event("move", viewport, mime, passing))
    # Dropped elsewhere than the last move: where it is dropped counts.
    over = screen(viewport, 0.25, 0.5)
    QApplication.sendEvent(viewport, drag_event("drop", viewport, mime, over))
    assert window.runner.active is None
    (new,) = set(references(document, "Mesa")) - set(before)
    x0, y0, _, _ = box(document, new)
    assert (round(x0, 2), round(y0, 2)) == (0.25, 0.5), "where it was dropped"
    assert len(document.undo_history) == steps + 1 and document.undo_name == "INSERT"


def test_a_block_dragged_away_inserts_nothing(window: MainWindow) -> None:
    opened(window)
    document = drawing_with_blocks(window, "Mesa")
    viewport = window.viewports[document.id]
    before, steps = references(document, "Mesa"), len(document.undo_history)
    mime = block_mime(document.id, "Mesa")
    QApplication.sendEvent(viewport, drag_event("enter", viewport, mime, QPointF(20, 20)))
    assert window.runner.active == "INSERT"
    QApplication.sendEvent(viewport, QDragLeaveEvent())
    assert window.runner.active is None
    assert references(document, "Mesa") == before and len(document.undo_history) == steps


def test_another_drawings_block_is_not_dropped_here(window: MainWindow) -> None:
    opened(window)
    document = drawing_with_blocks(window, "Mesa")
    viewport = window.viewports[document.id]
    mime = block_mime(document.id + 999, "Mesa")  # kept alive: the event points at it
    event = drag_event("enter", viewport, mime, QPointF(20, 20))
    QApplication.sendEvent(viewport, event)
    assert not event.isAccepted() and window.runner.active is None


def test_explode_inserts_the_blocks_contents(window: MainWindow) -> None:
    panel = opened(window)
    document = drawing_with_blocks(window, "Mesa")
    before = references(document, "Mesa")
    lines = sum(1 for e in document.get_entities() if e.kind == "LINE")
    steps = len(document.undo_history)
    panel.select("Mesa")
    panel.on_screen.setChecked(False)
    panel.explode.setChecked(True)
    panel.insert_button.click()
    assert references(document, "Mesa") == before, "no reference: its contents"
    assert sum(1 for e in document.get_entities() if e.kind == "LINE") == lines + 1
    assert len(document.undo_history) == steps + 1 and document.undo_name == "INSERT"
    document.undo()
    assert sum(1 for e in document.get_entities() if e.kind == "LINE") == lines


def test_the_drag_texts_speak_spanish(window: MainWindow) -> None:
    panel = opened(window)
    drawing_with_blocks(window, "Mesa")
    assert panel.drag_hint.isVisible() and panel.explode.text() == "Explode"
    set_language("es")
    try:
        panel.retranslate()
        assert panel.explode.text() == "Descomponer"
        assert panel.drag_hint.text() == "Arrastra un bloque al dibujo para insertarlo."
    finally:
        set_language("en")


def test_a_dropped_block_with_explode_leaves_its_contents_there(window: MainWindow) -> None:
    panel = opened(window)
    document = drawing_with_blocks(window, "Mesa")  # a line (0,0)-(0,1)
    viewport = window.viewports[document.id]
    viewport.zoom_extents()
    before = references(document, "Mesa")
    lines = {e.id for e in document.get_entities() if e.kind == "LINE"}
    panel.explode.setChecked(True)
    mime = block_mime(document.id, "Mesa")
    at = screen(viewport, 0.25, 0.5)
    QApplication.sendEvent(viewport, drag_event("enter", viewport, mime, at))
    QApplication.sendEvent(viewport, drag_event("drop", viewport, mime, at))
    assert window.runner.active is None and references(document, "Mesa") == before
    (line,) = {e.id for e in document.get_entities() if e.kind == "LINE"} - lines
    x0, y0, _, _ = box(document, line)
    assert (round(x0, 2), round(y0, 2)) == (0.25, 0.5), "its contents where it was dropped"
    assert document.undo_name == "INSERT"


# Step 7b: the Template source (ADR 0025 D1, D6, D7)


def wait_idle(qtbot: QtBot, window: MainWindow) -> None:
    qtbot.waitUntil(lambda: window.runner.busy is None, timeout=30_000)


def with_template(qtbot: QtBot, window: MainWindow, path: Path | None = None) -> BlocksPanel:
    """The palette with ``path`` (the sample template) loaded, as its empty
    state's button loads it."""
    panel = opened(window)
    assert isinstance(window.ui, QuietUi)
    window.ui.template_path = path if path is not None else sample_template()
    panel.set_source("template")
    panel.empty_load.click()
    wait_idle(qtbot, window)
    return panel


def history(window: MainWindow) -> list[str]:
    return window.command_window.history.toPlainText().splitlines()


def offered(panel: BlocksPanel) -> list[str]:
    """The templates the empty Template source offers, as buttons."""
    texts = []
    for i in range(panel.empty_recent.count()):
        item = panel.empty_recent.itemAt(i)
        button = item.widget() if item is not None else None
        assert isinstance(button, QPushButton)
        texts.append(button.text())
    return texts


def test_with_no_template_the_source_says_how_to_load_one(window: MainWindow) -> None:
    panel = opened(window)
    drawing_with_blocks(window, "Mesa")
    panel.set_source("template")
    assert panel.sources["template"].isChecked() and panel.source() == "template"
    assert panel.views.currentWidget() is panel.empty
    assert panel.empty_title.text() == "No template is loaded"
    assert "drop the file on this panel" in panel.empty_text.text()
    assert panel.template_row.isVisibleTo(panel) and not panel.source_caption.isVisibleTo(panel)
    assert offered(panel) == ["Sample template"], "the sample is always offered"
    assert panel.template_combo.currentIndex() == -1
    assert panel.template_combo.placeholderText() == "No template is loaded"
    assert panel.shown() == [] and not panel.insert_button.isEnabled()
    panel.set_source("drawing")
    assert panel.shown() == ["Mesa"] and panel.views.currentWidget() is panel.grid
    assert panel.source_caption.isVisibleTo(panel) and not panel.template_row.isVisibleTo(panel)


def test_a_template_loads_in_the_background_and_shows_its_blocks(
    qtbot: QtBot, window: MainWindow
) -> None:
    drawing_with_blocks(window, "Mesa")
    panel = opened(window)
    assert isinstance(window.ui, QuietUi)
    window.ui.template_path = sample_template()
    panel.set_source("template")
    panel.empty_load.click()
    assert window.ui.asked[-1] == "template", "the file dialog"
    assert window.runner.busy == "Loading template arqstudiocad-sample.dwt…"
    wait_idle(qtbot, window)
    template = panel.template
    assert template is not None and template not in window.session.documents, "never a tab"
    assert window.session.template is template, "BLOCK checks its names"
    assert sorted(panel.shown(), key=str.lower) == sorted(template.block_names, key=str.lower)
    assert len(panel.shown()) == 10 and panel.views.currentWidget() is panel.grid
    assert panel.template_combo.currentText() == "Sample template"
    assert history(window)[-1] == "Template arqstudiocad-sample.dwt loaded: 10 blocks."
    panel.select("Door_090")
    rows = dict(panel.details.sections()[0][1])
    assert rows["Source"] == "arqstudiocad-sample.dwt" and rows["In drawing"] == "Not inserted"
    assert not panel.edit_button.isEnabled(), "a template is never written"
    assert panel.edit_button.toolTip() == "Only the drawing's blocks can be edited."


def test_a_template_block_is_copied_and_inserted_in_one_step(
    qtbot: QtBot, window: MainWindow
) -> None:
    document = drawing_with_blocks(window, "Mesa")
    panel = with_template(qtbot, window)
    steps = len(document.undo_history)
    panel.select("Door_090")
    panel.on_screen.setChecked(False)
    panel.insert_button.click()
    assert "Door_090" in document.block_names
    assert len(references(document, "Door_090")) == 1
    assert "A-DOOR" in {layer.name for layer in document.get_layers()}, "with its layer"
    assert len(document.undo_history) == steps + 1 and document.undo_name == "INSERT"
    assert dict(panel.details.sections()[0][1])["In drawing"] == "1 reference"
    assert panel.template is not None and not panel.template.is_modified
    document.undo()
    assert "Door_090" not in document.block_names, "one undo takes the copy away too"
    assert references(document, "Door_090") == []


def test_a_template_block_dropped_on_the_drawing_lands_there(
    qtbot: QtBot, window: MainWindow
) -> None:
    document = drawing_with_blocks(window, "Mesa")
    panel = with_template(qtbot, window)
    viewport = window.viewports[document.id]
    viewport.zoom_extents()
    panel.select("Column_30x30")
    mime = panel.grid.mimeData([panel.grid.currentItem()])
    assert dragged_block(mime) == (document.id, "Column_30x30"), "for the active drawing"
    at = screen(viewport, 0.25, 0.5)
    QApplication.sendEvent(viewport, drag_event("enter", viewport, mime, at))
    assert window.runner.active == "INSERT"
    QApplication.sendEvent(viewport, drag_event("drop", viewport, mime, at))
    (placed,) = references(document, "Column_30x30")
    assert document.undo_name == "INSERT"
    document.undo()
    assert "Column_30x30" not in document.block_names and placed not in {
        e.id for e in document.get_entities()
    }


def test_a_clashing_template_block_is_asked_about_and_cancel_changes_nothing(
    qtbot: QtBot, window: MainWindow
) -> None:
    document = drawing_with_blocks(window, "wc")  # its own WC: one line
    panel = with_template(qtbot, window)
    steps = len(document.undo_history)
    panel.select("WC")
    # The drawing's own wc, counted whatever its case.
    assert dict(panel.details.sections()[0][1])["In drawing"] == "1 reference"
    panel.on_screen.setChecked(False)
    panel.insert_button.click()  # the dialog is cancelled
    assert isinstance(window.ui, QuietUi) and window.ui.asked[-1] == "clashes WC"
    (clash,) = window.ui.clashes
    assert (clash.name, clash.drawing_name, clash.suggested) == ("WC", "wc", "WC_2")
    assert clash.template is panel.template and clash.drawing is document
    assert not clash.dynamic
    assert "Inserting block WC." not in history(window), "cancelled before copying"
    assert len(document.undo_history) == steps and references(document, "WC") == []
    assert len(references(document, "wc")) == 1, "the drawing's own, untouched"


def test_a_template_file_dropped_on_the_palette_is_loaded(qtbot: QtBot, window: MainWindow) -> None:
    panel = opened(window)
    mime = QMimeData()
    mime.setUrls([QUrl.fromLocalFile(str(sample_template()))])
    enter = drag_event("enter", panel, mime, QPointF(10, 10))
    QApplication.sendEvent(panel, enter)
    assert enter.isAccepted()
    QApplication.sendEvent(panel, drag_event("drop", panel, mime, QPointF(10, 10)))
    wait_idle(qtbot, window)
    assert panel.source() == "template" and len(panel.shown()) == 10
    other = QMimeData()
    other.setUrls([QUrl.fromLocalFile("C:/plano.pdf")])
    refused = drag_event("enter", panel, other, QPointF(10, 10))
    QApplication.sendEvent(panel, refused)
    assert not refused.isAccepted(), "only drawings and templates"
    assert template_file("a.DWT") == Path("a.DWT") and template_file("a.txt") is None


def test_templates_used_are_remembered_and_offered(qtbot: QtBot, tmp_path: Path) -> None:
    settings = QSettings(str(tmp_path / "settings.ini"), QSettings.Format.IniFormat)
    template = tmp_path / "mine.dwt"
    template.write_bytes(sample_template().read_bytes())
    first = MainWindow(ui=QuietUi(), settings=settings)
    qtbot.addWidget(first)
    first.blocks_panel.load_template(template)
    wait_idle(qtbot, first)
    assert first.blocks_panel.template_combo.currentText() == "mine.dwt"
    first.blocks_panel.load_template(sample_template())
    wait_idle(qtbot, first)
    stored = settings.value(TEMPLATES_SETTING)
    assert isinstance(stored, list) and len(stored) == 2
    second = MainWindow(ui=QuietUi(), settings=settings)
    qtbot.addWidget(second)
    combo = second.blocks_panel.template_combo
    assert [combo.itemText(i) for i in range(combo.count())] == ["Sample template", "mine.dwt"]
    assert second.blocks_panel.template is None, "offered, not loaded"
    combo.activated.emit(1)  # chosen in the combo
    wait_idle(qtbot, second)
    template_doc = second.blocks_panel.template
    assert template_doc is not None and template_doc.name == "mine.dwt"


def test_a_file_that_is_no_template_leaves_the_loaded_one(
    qtbot: QtBot, window: MainWindow, tmp_path: Path
) -> None:
    panel = with_template(qtbot, window)
    loaded = panel.template
    broken = tmp_path / "roto.dwt"
    broken.write_bytes(b"not a drawing")
    panel.load_template(broken)
    wait_idle(qtbot, window)
    assert panel.template is loaded and len(panel.shown()) == 10
    assert history(window)[-1].startswith("roto.dwt cannot be loaded as a template")


def test_a_new_template_replaces_and_closes_the_old_one(
    qtbot: QtBot, window: MainWindow, tmp_path: Path
) -> None:
    panel = with_template(qtbot, window)
    old = panel.template
    assert old is not None
    copy = tmp_path / "otra.dwt"
    copy.write_bytes(sample_template().read_bytes())
    panel.load_template(copy)
    wait_idle(qtbot, window)
    assert panel.template is not old and not old.is_open, "closed in the core"


def test_the_template_source_speaks_spanish(qtbot: QtBot, window: MainWindow) -> None:
    panel = opened(window)
    panel.set_source("template")
    set_language("es")
    try:
        window.retranslate()
        assert panel.sources["template"].text() == "Plantilla"
        assert panel.empty_title.text() == "No hay ninguna plantilla cargada"
        assert panel.empty_load.text() == "Cargar plantilla…"
        assert panel.empty_recent_label.text() == "Usadas hace poco"
        assert offered(panel) == ["Plantilla de ejemplo"]
        assert panel.load_button.toolTip() == "Cargar plantilla…"
    finally:
        set_language("en")


# Step 7c: the clash dialog (ADR 0025 D6)


def clash_answered(qtbot: QtBot, window: MainWindow, answer: ClashChoice) -> CadDocument:
    """A drawing with its own wc and the sample template loaded; the clash
    dialog will answer ``answer`` for WC."""
    document = drawing_with_blocks(window, "wc")
    with_template(qtbot, window)
    assert isinstance(window.ui, QuietUi)
    window.ui.clash_answer = {"WC": answer}
    return document


def insert_wc(window: MainWindow) -> None:
    panel = window.blocks_panel
    panel.select("WC")
    panel.on_screen.setChecked(False)
    panel.insert_button.click()


def test_copy_with_another_name_inserts_the_renamed_copy(qtbot: QtBot, window: MainWindow) -> None:
    document = clash_answered(qtbot, window, ClashChoice("rename", "WC_2"))
    steps = len(document.undo_history)
    insert_wc(window)
    assert sorted(document.block_names, key=str.lower) == ["wc", "WC_2"]
    assert len(references(document, "WC_2")) == 1 and len(references(document, "wc")) == 1
    assert len(document.undo_history) == steps + 1 and document.undo_name == "INSERT"
    document.undo()
    assert document.block_names == ["wc"], "one undo takes the copy away too"


def test_redefine_replaces_the_drawings_block(qtbot: QtBot, window: MainWindow) -> None:
    document = clash_answered(qtbot, window, ClashChoice("redefine"))
    template = window.blocks_panel.template
    assert template is not None
    insert_wc(window)
    assert document.block_clashes("WC", template) == [], "now the template's"
    assert document.block_names == ["wc"] and len(references(document, "wc")) == 2
    document.undo()
    assert document.block_clashes("WC", template) == ["WC"], "undone in one step"


def test_use_the_drawings_inserts_its_own_block(qtbot: QtBot, window: MainWindow) -> None:
    document = clash_answered(qtbot, window, ClashChoice("keep"))
    template = window.blocks_panel.template
    assert template is not None
    insert_wc(window)
    assert document.block_clashes("WC", template) == ["WC"], "the drawing's, unchanged"
    assert len(references(document, "wc")) == 2


def test_a_clashing_block_dropped_is_asked_about_then_placed_there(
    qtbot: QtBot, window: MainWindow
) -> None:
    document = clash_answered(qtbot, window, ClashChoice("rename", "WC_2"))
    panel = window.blocks_panel
    viewport = window.viewports[document.id]
    viewport.zoom_extents()
    panel.select("WC")
    mime = panel.grid.mimeData([panel.grid.currentItem()])
    at = screen(viewport, 0.25, 0.5)
    QApplication.sendEvent(viewport, drag_event("enter", viewport, mime, at))
    assert window.runner.active is None, "no dialog during the drag: asked once dropped"
    QApplication.sendEvent(viewport, drag_event("drop", viewport, mime, at))
    qtbot.waitUntil(lambda: bool(references(document, "WC_2")))
    assert isinstance(window.ui, QuietUi) and window.ui.asked[-1] == "clashes WC"
    (placed,) = references(document, "WC_2")
    x0, y0, x1, y1 = box(document, placed)
    assert x0 <= 0.25 <= x1 and y0 <= 0.5 <= y1, "where it was dropped"
    assert document.undo_name == "INSERT"


def test_a_clashing_dynamic_block_can_only_use_the_drawings(
    qtbot: QtBot, window: MainWindow
) -> None:
    from test_block_editor import LOOKUP
    from test_commands import regression_drawing

    path = regression_drawing(LOOKUP)
    template = window.session.app.open_document(path)
    name = next(n for n in template.block_names if template.is_dynamic_block(n))
    drawing_with_blocks(window, name)
    panel = with_template(qtbot, window, path)
    assert isinstance(window.ui, QuietUi)
    panel.select(name)
    panel.on_screen.setChecked(False)
    panel.insert_button.click()
    (clash,) = window.ui.clashes
    assert clash.dynamic and clash.name == name


# The Recent source (ADR 0025 D1, step 6)


def shown_window(qtbot: QtBot, settings: QSettings) -> MainWindow:
    w = MainWindow(ui=QuietUi(), settings=settings)
    qtbot.addWidget(w)
    w.resize(1100, 700)
    w.show()
    qtbot.waitExposed(w)
    return w


def recent_rows(panel: BlocksPanel) -> list[tuple[str, str]]:
    """Each Recent row's name and how it was used, as its tooltip says."""
    return [
        (panel._row(i).text(0), panel._row(i).toolTip(0).splitlines()[1].split(" · ")[0])
        for i in range(panel.table.topLevelItemCount())
    ]


def details(panel: BlocksPanel) -> dict[str, str]:
    return dict(panel.details.sections()[0][1])


def block_made(window: MainWindow, document: CadDocument, name: str) -> None:
    """BLOCK typed: a line made into block ``name``."""
    line, _ = document.add_line((0, 0), (2, 3))
    window.session.select(document, [line])
    for typed in ("BLOCK", name, "0,0"):
        window.runner.execute(typed)


def insert_at_origin(panel: BlocksPanel) -> None:
    panel.on_screen.setChecked(False)
    panel.insert_button.click()


def test_recent_lists_the_blocks_created_and_inserted_newest_first(
    qtbot: QtBot, window: MainWindow
) -> None:
    panel = opened(window)
    first = drawing_with_blocks(window, "Mesa")
    block_made(window, first, "Silla")
    panel.select("Mesa")
    insert_at_origin(panel)
    panel.set_source("recent")
    assert panel.sources["recent"].isChecked() and panel.source() == "recent"
    assert recent_rows(panel) == [("Mesa", "Inserted here"), ("Silla", "Created here")]
    assert panel.table.headerItem().text(1) == "Used" and panel._row(0).text(1) == "Just now"
    assert panel.footer.text() == "2 blocks"
    assert panel.source_caption.text() == (
        "Blocks created or inserted in this session and the last."
    )
    assert not panel.template_row.isVisibleTo(panel)
    assert not panel._row(0).icon(0).isNull(), "its preview"
    panel.select("Silla")
    assert details(panel)["Used"] == "Created here · Just now"
    steps = len(first.undo_history)
    insert_at_origin(panel)
    assert first.undo_history[steps:] == ["INSERT"] and len(references(first, "Silla")) == 2, (
        "the drawing's own block: inserted, not copied"
    )

    second = window.session.new_document()
    assert recent_rows(panel) == [
        ("Silla", f"Inserted from {first.name}"),
        ("Mesa", f"Inserted from {first.name}"),
    ]
    panel.select("Silla")
    assert panel.insert_button.isEnabled() and not panel.edit_button.isEnabled()
    assert details(panel)["Used"] == f"Inserted from {first.name} · Just now"
    steps = len(second.undo_history)
    insert_at_origin(panel)
    assert "Silla" in second.block_names, "copied from the drawing it was made in"
    assert second.undo_history[steps:] == ["INSERT"], "one undo step"
    assert recent_rows(panel)[0] == ("Silla", f"Inserted from {first.name}")
    assert len(recent_rows(panel)) == 2, "the same block of the same drawing, once"
    panel.search.setText("mes")
    assert panel.shown() == ["Mesa"] and panel.footer.text() == "1 block"
    assert panel.names() == ["Mesa"], "the recent blocks, not the drawing's"
    panel.search.setText("ventana")
    assert panel.footer.text() == "No block matches the search."
    panel.search.clear()
    panel.set_source("drawing")
    assert panel.table.headerItem().text(1) == "In drawing"


def test_recent_starts_empty_and_says_so(window: MainWindow) -> None:
    panel = opened(window)
    panel.set_source("recent")
    assert panel.shown() == []
    assert panel.footer.text() == "No block was created or inserted lately."


def test_recent_blocks_are_kept_for_this_session_and_the_next(qtbot: QtBot, tmp_path: Path) -> None:
    settings = QSettings(str(tmp_path / "settings.ini"), QSettings.Format.IniFormat)
    first = shown_window(qtbot, settings)
    assert first.show_panel("blocks")
    planta = first.session.new_document()
    block_made(first, planta, "Mesa")
    first.session.save(planta, tmp_path / "planta.dwg")
    unsaved = first.session.new_document()
    block_made(first, unsaved, "Boceto")
    first.blocks_panel.set_source("recent")
    assert first.blocks_panel.shown() == ["Boceto", "Mesa"]
    (mesa,) = [b for b in first.blocks_panel.recent.blocks if b.name == "Mesa"]
    assert mesa.path == tmp_path / "planta.dwg"
    assert first.blocks_panel.recent_source(mesa) == planta, "the open drawing, not its file"
    first.session.close(unsaved)
    assert first.blocks_panel.shown() == ["Mesa"], "no file holds Boceto: it leaves"
    assert int(str(settings.value(SESSION_SETTING))) == 1

    second = shown_window(qtbot, settings)
    panel = second.blocks_panel
    assert second.show_panel("blocks")
    target = second.session.new_document()
    panel.set_source("recent")
    assert second.runner.busy == "Reading the recent blocks' files…", "in the background"
    wait_idle(qtbot, second)
    (mesa,) = panel.recent.blocks
    source = panel.recent_source(mesa)
    assert source is not None and source.path == tmp_path / "planta.dwg"
    assert source not in second.session.documents, "read, never in a tab"
    assert not panel._row(0).icon(0).isNull(), "its preview"
    assert recent_rows(panel) == [("Mesa", "Created in planta.dwg")]
    panel.select("Mesa")
    insert_at_origin(panel)
    assert "Mesa" in target.block_names
    assert recent_rows(panel)[0] == ("Mesa", "Inserted from planta.dwg")
    assert not source.is_modified, "the file read is never written"
    opened_tab = second.session.open_document(tmp_path / "planta.dwg")
    assert panel.recent_source(panel.recent.blocks[0]) == opened_tab, "open now: the tab's"

    third = shown_window(qtbot, settings)
    assert [b.name for b in third.blocks_panel.recent.blocks] == ["Mesa"], "the previous session's"
    fourth = shown_window(qtbot, settings)
    assert fourth.blocks_panel.recent.blocks == [], "two sessions ago: forgotten"
    assert int(str(settings.value(SESSION_SETTING))) == 4


def stored_recent(settings: QSettings, *blocks: tuple[str, Path]) -> None:
    """The settings as the previous session left them, the newest first."""
    settings.setValue(SESSION_SETTING, 1)
    settings.setValue(
        RECENT_SETTING,
        json.dumps(
            [
                {"name": n, "path": str(p), "how": "inserted", "when": 1.0, "session": 1}
                for n, p in blocks
            ]
        ),
    )


def test_a_recent_block_that_cannot_be_had_is_shown_and_says_why(
    qtbot: QtBot, tmp_path: Path
) -> None:
    settings = QSettings(str(tmp_path / "settings.ini"), QSettings.Format.IniFormat)
    mine = tmp_path / "mine.dwt"
    mine.write_bytes(sample_template().read_bytes())
    gone = tmp_path / "gone.dwg"
    stored_recent(settings, ("Tree", mine), ("Nada", mine), ("Mesa", gone))
    window = shown_window(qtbot, settings)
    panel = opened(window)
    document = drawing_with_blocks(window, "Mesa")
    panel.set_source("recent")
    wait_idle(qtbot, window)
    assert panel.shown() == ["Tree", "Nada", "Mesa"], "every one shown"
    reasons = {}
    for name in ("Tree", "Nada", "Mesa"):
        assert panel.select(name)
        reasons[name] = panel.insert_button.toolTip()
        assert panel.insert_button.isEnabled() == (name == "Tree")
    assert reasons == {
        "Tree": "",
        "Nada": "mine.dwt no longer has a block named Nada.",
        "Mesa": f"{gone} was not found.",
    }
    muted = panel.palette().color(QPalette.ColorRole.PlaceholderText)
    assert panel._row(2).foreground(0).color() == muted, "greyed"
    assert panel._row(0).foreground(0).color() != muted
    assert panel._row(2).toolTip(0).endswith(f"{gone} was not found.")
    assert panel._row(2).icon(0).isNull(), "no preview to draw"
    assert details(panel)["Cannot be inserted"] == f"{gone} was not found."
    assert not panel.grid.mimeData([panel.grid.item(2)]).hasFormat(BLOCK_MIME), "no drag"
    panel.insert_button.setEnabled(True)
    steps = len(document.undo_history)
    panel.insert_selected()
    assert len(document.undo_history) == steps, "refused, not the drawing's own Mesa"

    read = panel.recent_source(panel.recent.blocks[0])
    assert read is not None
    stat = mine.stat()
    os.utime(mine, (stat.st_atime, stat.st_mtime + 10))
    panel.refresh()
    wait_idle(qtbot, window)
    again = panel.recent_source(panel.recent.blocks[0])
    assert again is not None and again != read and not read.is_open, "changed: read again"


def test_a_file_that_cannot_be_read_says_so_and_is_tried_again(
    qtbot: QtBot, tmp_path: Path
) -> None:
    settings = QSettings(str(tmp_path / "settings.ini"), QSettings.Format.IniFormat)
    broken = tmp_path / "broken.dwg"
    broken.write_bytes(b"not a drawing")
    stored_recent(settings, ("Mesa", broken))
    window = shown_window(qtbot, settings)
    panel = opened(window)
    window.session.new_document()
    panel.set_source("recent")
    wait_idle(qtbot, window)
    assert panel.select("Mesa") and not panel.insert_button.isEnabled()
    assert panel.insert_button.toolTip().startswith("broken.dwg cannot be read: ")
    panel.refresh()
    assert window.runner.busy is None, "not read again at each refresh"
    broken.write_bytes(sample_template().read_bytes())
    panel.set_source("recent")
    wait_idle(qtbot, window)
    assert panel.select("Mesa")
    assert panel.insert_button.toolTip() == "broken.dwg no longer has a block named Mesa."


def test_a_recent_block_whose_file_is_not_read_yet_is_read_when_used(
    qtbot: QtBot, tmp_path: Path
) -> None:
    settings = QSettings(str(tmp_path / "settings.ini"), QSettings.Format.IniFormat)
    other = tmp_path / "other.dwt"
    other.write_bytes(sample_template().read_bytes())
    stored_recent(settings, ("Tree", sample_template()), ("Column_30x30", other))
    window = shown_window(qtbot, settings)
    panel = opened(window)
    document = window.session.new_document()
    window.runner.execute("LINE")  # at work: no file is read in its way
    panel.set_source("recent")
    assert window.runner.active == "LINE" and window.runner.busy is None
    assert panel.recent_source(panel.recent.blocks[0]) is None
    assert panel.select("Tree") and panel.insert_button.isEnabled(), "read on insert"
    steps = len(document.undo_history)
    insert_at_origin(panel)
    wait_idle(qtbot, window)
    assert "Tree" in document.block_names and document.undo_history[steps:] == ["INSERT"]
    assert panel.recent_source(panel.recent.blocks[0]) is not None, "kept for its preview"

    window.runner.execute("LINE")
    panel.refresh()
    (column,) = [b for b in panel.recent.blocks if b.name == "Column_30x30"]
    assert panel.recent_source(column) is None, "not read yet"
    assert panel.select("Column_30x30")
    mime = panel.grid.mimeData([panel.grid.currentItem()])
    assert dragged_block(mime) == (document.id, "Column_30x30")
    panel.start_drop_insert("Column_30x30")
    assert window.runner.active == "LINE", "nothing is read during the drag"
    panel.drop_insert_at((50.0, 60.0))
    panel.end_drop_insert(True)
    qtbot.waitUntil(lambda: len(references(document, "Column_30x30")) == 1, timeout=30_000)
    wait_idle(qtbot, window)
    (placed,) = references(document, "Column_30x30")
    x0, y0, x1, y1 = box(document, placed)
    assert x0 <= 50 <= x1 and y0 <= 60 <= y1, "where it was dropped"


def test_two_recent_blocks_of_one_name_are_told_apart(qtbot: QtBot, tmp_path: Path) -> None:
    settings = QSettings(str(tmp_path / "settings.ini"), QSettings.Format.IniFormat)
    other = tmp_path / "other.dwt"
    other.write_bytes(sample_template().read_bytes())
    stored_recent(settings, ("WC", sample_template()), ("WC", other))
    window = shown_window(qtbot, settings)
    panel = opened(window)
    window.session.new_document()
    panel.set_source("recent")
    wait_idle(qtbot, window)
    assert panel.shown() == ["WC", "WC"]
    panel.grid.setCurrentRow(1)
    chosen = panel.selected_recent()
    assert chosen is not None and chosen.path == other, "the card's own row"
    assert panel.table.currentItem() is panel._row(1)
    panel.table.setCurrentItem(panel._row(0))
    assert panel.grid.currentRow() == 0, "the grid follows the list"
    panel.table.setCurrentItem(panel._row(1))
    panel.refresh()
    chosen = panel.selected_recent()
    assert chosen is not None and chosen.path == other, "kept across a refresh"
    assert details(panel)["Source"] == "other.dwt"
    assert details(panel)["Used"].startswith("Inserted from other.dwt")
