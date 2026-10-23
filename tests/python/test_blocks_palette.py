"""The Blocks palette (ADR 0025): the drawing's blocks laid out as the
mockup, the selected block's details, and a View-tab button that toggles
the palette and is pressed while it is open (D2)."""

from pathlib import Path

import pytest
from PySide6.QtCore import QSettings, QSize, Qt
from PySide6.QtGui import QColor, QIcon, QImage, QPalette
from PySide6.QtWidgets import QMenu, QToolButton
from pytestqt.qtbot import QtBot
from ui_fakes import QuietUi

from arqstudiocad.core import CadChangeSet, CadDocument
from arqstudiocad.i18n import set_language
from arqstudiocad.ui.blocks_panel import (
    TAG_BACKGROUND,
    TAG_IN_ROWS,
    TAG_ON_CARDS,
    TAG_STAR,
    BlocksPanel,
    attributes_text,
    references_text,
    units_name,
)
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
        ("Template", False, False),
        ("Recent", False, False),
        ("Drawing", True, True),
    ], "only the drawing's blocks yet"
    assert panel.sources["template"].toolTip() == "Not available yet."
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
    data = document.block_preview("mesa", 32, (255, 0, 0))
    assert data is not None and len(data) == 32 * 32 * 4
    pixels = [data[i : i + 4] for i in range(0, len(data), 4)]
    assert any(px[3] for px in pixels) and pixels[0][3] == 0, "ink on transparency"
    assert document.block_preview("nada", 32, (255, 0, 0)) is None
    with pytest.raises(ValueError, match="1 to 1024"):
        document.block_preview("Mesa", 0, (255, 0, 0))


def cards(panel: BlocksPanel) -> list[str]:
    return [panel.grid.item(i).text() for i in range(panel.grid.count())]


def test_the_grid_shows_cards_with_previews(window: MainWindow) -> None:
    panel = opened(window)
    drawing_with_blocks(window, "Mesa", "Silla", "Puerta")
    assert panel.views.currentWidget() is panel.grid
    assert cards(panel) == ["Mesa", "Puerta", "Silla"] == shown(window)
    card = panel.grid.item(0)
    size = panel.grid.card_size()
    plain = card.icon().pixmap(QSize(size, size), QIcon.Mode.Normal).toImage()
    chosen = card.icon().pixmap(QSize(size, size), QIcon.Mode.Selected).toImage()
    assert not plain.isNull() and plain != chosen, "accent strokes when selected"

    def ink(image: QImage) -> QColor:
        """The color of the most opaque pixel: the stroke's."""
        pixels = [
            image.pixelColor(x, y) for x in range(image.width()) for y in range(image.height())
        ]
        return max(pixels, key=lambda c: c.alpha())

    def near(a: QColor, b: QColor) -> bool:
        return (
            max(abs(a.red() - b.red()), abs(a.green() - b.green()), abs(a.blue() - b.blue())) <= 4
        )

    palette = panel.palette()
    assert near(ink(chosen), palette.color(QPalette.ColorRole.Highlight)), "the theme's accent"
    assert near(ink(plain), palette.color(QPalette.ColorRole.Text)), "the theme's text color"
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
