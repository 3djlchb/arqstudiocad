"""The block editor (ADR 0023 D3): BEDIT opens a block's definition alone in
a document of its own; BSAVE, SAVE, BCLOSE and CLOSE put it back into the
drawing, never into a file; a contextual ribbon tab while it is active."""

from pathlib import Path

import pytest
from PySide6.QtCore import QEvent, QPointF, Qt
from PySide6.QtGui import QMouseEvent
from PySide6.QtWidgets import QApplication
from pytestqt.qtbot import QtBot
from test_commands import regression_drawing
from ui_fakes import QuietUi

from arqstudiocad.commands import CommandContext, CommandRunner
from arqstudiocad.commands.blocks import edit_named, insert_named
from arqstudiocad.commands.builtin import builtin_registry
from arqstudiocad.core import CadApplication, CadDocument, InvalidOperationError
from arqstudiocad.events import BlockSaved, CommandMessage, CommandPrompt, Event, EventBus
from arqstudiocad.i18n import set_language
from arqstudiocad.session import Session
from arqstudiocad.ui.main_window import MainWindow
from arqstudiocad.ui.viewport import BLOCK_EDITOR_BACKGROUND, Viewport

LOOKUP = Path(__file__).resolve().parents[2] / (
    "target/corpus/acadrust/fixtures/dynamic-blocks/lookup.dwg"
)


def drawn_box(document: CadDocument) -> tuple[tuple[float, float], tuple[float, float]]:
    """The box of the first entity, rounded: the reference to MARCO."""
    bounds = document.get_entities()[0].bounds
    assert bounds is not None
    r = round
    return (r(bounds.min[0], 6), r(bounds.min[1], 6)), (r(bounds.max[0], 6), r(bounds.max[1], 6))


def framed(document: CadDocument) -> None:
    """Block MARCO, based at (10,10): (10,10)-(14,10) and (10,10)-(10,13),
    referenced where they were."""
    a, _ = document.add_line((10, 10), (14, 10))
    b, _ = document.add_line((10, 10), (10, 13))
    document.create_block("MARCO", (10, 10), [a, b])


def test_the_api_opens_and_saves_a_block_editor() -> None:
    doc = CadApplication().create_document("Plano")
    framed(doc)
    editor = doc.open_block_editor("MARCO", "MARCO en Plano")
    assert editor.name == "MARCO en Plano" and editor.path is None
    assert [e.kind for e in editor.get_entities()] == ["LINE", "LINE"]
    assert not editor.is_modified and not doc.is_dynamic_block("MARCO")
    editor.add_line((10, 10), (10, 20))
    changes = doc.save_block("MARCO", editor)
    assert len(changes.modified) == 1 and not editor.is_modified
    assert drawn_box(doc) == ((10.0, 10.0), (14.0, 20.0))
    assert doc.undo_name == "BEDIT"


class Harness:
    def __init__(self, ui: QuietUi | None = None) -> None:
        self.bus = EventBus()
        self.events: list[Event] = []
        self.bus.subscribe(Event, self.events.append)
        self.session = Session(self.bus)
        self.runner = CommandRunner(builtin_registry(), CommandContext(self.session, ui=ui))
        self.document = self.session.new_document()
        framed(self.document)

    def run(self, *lines: str) -> None:
        for line in lines:
            self.runner.execute(line)

    def active(self) -> CadDocument:
        assert self.session.active is not None
        return self.session.active

    def prompt(self) -> str:
        return [e for e in self.events if isinstance(e, CommandPrompt)][-1].text

    def messages(self) -> list[str]:
        return [e.text for e in self.events if isinstance(e, CommandMessage)]


def test_bedit_opens_the_block_in_a_document_of_its_own() -> None:
    h = Harness()
    h.run("BEDIT", "marco")
    editor = h.active()
    assert editor != h.document and editor.name == "MARCO in Drawing1"
    assert h.session.block_editor_of(editor) == (h.document, "MARCO")
    assert h.messages()[-1] == "Editing block MARCO."
    assert len(editor.get_entities()) == 2
    h.session.activate(h.document)
    h.run("BE", "MARCO")
    assert h.active() == editor, "the editor already open"
    h.run("BEDIT")
    assert h.messages()[-1] == "Close this block editor before editing another block."


def test_bsave_and_save_put_the_block_back_as_one_step() -> None:
    h = Harness()
    h.run("BEDIT", "MARCO")
    editor = h.active()
    editor.add_line((10, 10), (10, 20))
    h.run("BSAVE")
    assert drawn_box(h.document) == ((10.0, 10.0), (14.0, 20.0))
    assert h.messages()[-1] == "Block MARCO saved in Drawing1."
    assert not editor.is_modified and h.active() == editor, "the editor stays open"
    saved = [e for e in h.events if isinstance(e, BlockSaved)]
    assert saved == [BlockSaved(editor.id, h.document.id, "MARCO")]
    editor.add_line((10, 10), (20, 10))
    h.run("SAVE")
    assert drawn_box(h.document) == ((10.0, 10.0), (20.0, 20.0)), "SAVE saves the block"
    assert editor.path is None, "never a file"
    h.run("SAVEAS")
    assert h.messages()[-1].startswith("A block editor is saved into its drawing")
    h.document.undo()
    assert drawn_box(h.document) == ((10.0, 10.0), (14.0, 20.0)), "each save one step"


def test_closing_asks_to_save_the_block_and_returns_to_the_drawing() -> None:
    h = Harness()
    h.run("BEDIT", "MARCO")
    h.active().add_line((10, 10), (10, 20))
    h.run("BCLOSE")
    assert h.prompt() == "Save changes to MARCO in Drawing1? [Yes(Y)/No(N)] <Y>:"
    h.run("N")
    assert h.active() == h.document and drawn_box(h.document) == ((10.0, 10.0), (14.0, 13.0))
    h.run("BEDIT", "MARCO")
    h.active().add_line((10, 10), (10, 20))
    h.run("CLOSE", "Y")
    assert h.active() == h.document and drawn_box(h.document) == ((10.0, 10.0), (14.0, 20.0))
    h.run("BCLOSE")
    assert h.messages()[-1] == "This is not a block editor."
    # With another drawing opened between them, back to its own drawing.
    other = h.session.new_document()
    h.session.activate(h.document)
    h.run("BEDIT", "MARCO", "BCLOSE")
    assert h.active() == h.document != other


def test_closing_the_drawing_closes_its_editors_first() -> None:
    h = Harness()
    h.run("BEDIT", "MARCO")
    editor = h.active()
    editor.add_line((10, 10), (10, 20))
    h.session.activate(h.document)
    h.run("CLOSE")
    assert h.prompt() == "Save changes to MARCO in Drawing1? [Yes(Y)/No(N)] <Y>:"
    h.run("Y")
    assert not editor.is_open
    assert h.prompt() == "Save changes to Drawing1? [Yes(Y)/No(N)] <Y>:", "then the drawing"
    h.runner.cancel()
    assert h.session.documents == [h.document]


def test_a_block_cannot_hold_itself() -> None:
    h = Harness()
    h.run("BEDIT", "MARCO")
    h.active().insert_block("MARCO", (0, 0))
    h.run("BSAVE")
    assert h.messages()[-1] == "MARCO cannot hold a reference to itself."
    assert drawn_box(h.document) == ((10.0, 10.0), (14.0, 13.0))


def test_bedit_refuses_dynamic_blocks_and_drawings_without_blocks() -> None:
    h = Harness()
    h.session.activate(h.session.new_document())
    h.run("BEDIT")
    assert h.messages()[-1] == "The drawing has no blocks to edit."
    h.session.activate(h.session.open_document(regression_drawing(LOOKUP)))
    h.run("BEDIT", "My_Look_Block")
    assert h.messages()[-1] == "My_Look_Block is a dynamic block: it cannot be edited yet."
    assert h.session.block_editor_of(h.active()) is None
    with pytest.raises(InvalidOperationError):
        h.active().open_block_editor("My_Look_Block", "x")  # the core refuses it too


def test_bedit_chooses_in_a_window_and_answers_in_windows() -> None:
    ui = QuietUi(block_answer="MARCO", save_changes="yes")
    h = Harness(ui)
    h.run("BEDIT")
    assert ui.asked[-1] == "Edit block: MARCO"
    h.active().add_line((10, 10), (10, 20))
    h.run("BCLOSE")
    assert ui.asked[-1] == "changes MARCO in Drawing1"
    assert drawn_box(h.document) == ((10.0, 10.0), (14.0, 20.0))


def test_block_editor_commands_in_spanish() -> None:
    h = Harness()
    set_language("es")
    for typed, english in (
        ("EDITARBLOQUE", "BEDIT"),
        ("GUARDARBLOQUE", "BSAVE"),
        ("CERRARBLOQUE", "BCLOSE"),
    ):
        found = h.runner.registry.find(typed)
        assert found is not None and found.name == english


@pytest.fixture
def window(qtbot: QtBot) -> MainWindow:
    w = MainWindow(ui=QuietUi())
    qtbot.addWidget(w)
    return w


def test_the_block_editor_tab_shows_only_while_editing(window: MainWindow) -> None:
    from PySide6.QtWidgets import QMenu

    from arqstudiocad.ui.ribbon import Ribbon

    ribbon = window.findChildren(Ribbon)[0]

    def shown() -> list[str]:
        return [ribbon.tabText(i) for i in range(ribbon.count()) if ribbon.isTabVisible(i)]

    document = window.session.new_document()
    framed(document)
    assert "Block Editor" not in shown()
    editor = window.session.open_block_editor(document, "MARCO")
    assert shown()[-1] == "Block Editor"
    assert ribbon.tabText(ribbon.currentIndex()) == "Block Editor", "brought forward"
    editor_tab = window.tabs.currentIndex()
    assert window.tabs.tabText(editor_tab) == "MARCO in Drawing1"
    editor.add_line((0, 0), (1, 1))
    window.session.report(editor, editor.add_line((0, 0), (2, 2))[1])
    assert window.tabs.tabText(editor_tab) == "MARCO in Drawing1*"
    window.session.save_block(editor)
    assert window.tabs.tabText(editor_tab) == "MARCO in Drawing1", "saved"
    window.session.activate(document)
    assert "Block Editor" not in shown()
    assert ribbon.tabText(ribbon.currentIndex()) == "Home", "left with the context"
    actions = window.command_actions
    menus = {m.title(): m for m in window.menuBar().findChildren(QMenu)}
    for name in ("BEDIT", "BSAVE", "BCLOSE"):
        assert actions[name] in menus["&Insert"].actions()
    assert (actions["BSAVE"].text(), actions["BCLOSE"].text()) == (
        "&Save Block",
        "C&lose Block Editor",
    )


def test_the_palette_starts_bedit_and_insert_with_the_block_chosen() -> None:
    """ADR 0025 D5: the same commands, the block already named."""
    h = Harness()
    h.runner.start(insert_named("nada", 1.0, 0.0, False))
    assert h.messages()[-1] == "There is no such block."
    h.runner.start(insert_named("marco", 1.0, 0.0, True))
    assert h.messages()[-1] == "Inserting block MARCO.", "spelled as the drawing does"
    assert h.prompt().startswith("Specify insertion point")
    h.run("20,0")
    assert h.runner.active is None and h.document.undo_name == "INSERT"
    h.runner.start(edit_named("MARCO"))
    editor = h.active()
    assert h.session.block_editor_of(editor) == (h.document, "MARCO")
    h.runner.start(edit_named("MARCO"))
    assert h.messages()[-1] == "Close this block editor before editing another block."
    h.session.activate(h.document)
    h.runner.start(edit_named("nada"))
    assert h.messages()[-1] == "There is no such block."


def double_click(viewport: Viewport, world: tuple[float, float]) -> None:
    point = QPointF(*viewport.view.to_screen(*world))
    event = QMouseEvent(
        QEvent.Type.MouseButtonDblClick,
        point,
        viewport.mapToGlobal(point),
        Qt.MouseButton.LeftButton,
        Qt.MouseButton.LeftButton,
        Qt.KeyboardModifier.NoModifier,
    )
    QApplication.sendEvent(viewport, event)


def test_a_double_click_on_a_block_edits_it_in_a_grey_editor(qtbot: QtBot) -> None:
    """As the maintainer's video (2026-10-25): a double click on a block
    opens the Edit Block Definition dialog with that block chosen, then the
    block editor, in a tab of its own on a grey background."""
    ui = QuietUi(block_answer="MARCO")
    window = MainWindow(ui=ui)
    qtbot.addWidget(window)
    window.resize(900, 600)
    window.show()
    qtbot.waitExposed(window)
    document = window.session.new_document()
    a, _ = document.add_line((0, 0), (40, 0))
    b, _ = document.add_line((0, 0), (0, 30))
    window.session.edit(document, lambda d: d.create_block("MARCO", (0, 0), [a, b])[1])
    window.session.edit(document, lambda d: d.add_line((80, 80), (100, 100))[1])
    viewport = window.viewports[document.id]
    viewport.zoom_extents()
    asked = len(ui.asked)

    double_click(viewport, (90.0, 90.0))
    assert len(ui.asked) == asked, "a line: nothing to edit yet"
    window.runner.execute("BLOCK")  # asks for a name, not a point
    double_click(viewport, (20.0, 0.0))
    assert len(ui.asked) == asked and window.runner.active == "BLOCK", "not during a command"
    window.runner.cancel()

    double_click(viewport, (20.0, 0.0))
    assert ui.asked[-1] == "Edit block: MARCO" and ui.edit_chosen == "MARCO"
    editor = window.session.active
    assert editor is not None and window.session.block_editor_of(editor) == (document, "MARCO")
    assert editor.path is None, "a temporary tab, never a file"
    assert window.viewports[editor.id].view.background == BLOCK_EDITOR_BACKGROUND
    assert viewport.view.background != BLOCK_EDITOR_BACKGROUND, "the drawing keeps its own"
