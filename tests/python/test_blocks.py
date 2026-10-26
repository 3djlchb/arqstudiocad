"""BLOCK and INSERT (ADR 0023 D2): the core API, the commands and their
place in the interface (the Insert ribbon tab, D4)."""

import math
from pathlib import Path

import pytest
from PySide6.QtWidgets import QMessageBox, QWidget
from pytestqt.qtbot import QtBot
from ui_fakes import QuietUi

from arqstudiocad.commands import CommandContext, CommandRunner
from arqstudiocad.commands.blocks import insert_named
from arqstudiocad.commands.builtin import builtin_registry
from arqstudiocad.core import CadApplication, CadDocument, InvalidOperationError
from arqstudiocad.events import BlockUsed, CommandMessage, CommandPrompt, Event, EventBus
from arqstudiocad.i18n import set_language
from arqstudiocad.paths import sample_template
from arqstudiocad.session import Session
from arqstudiocad.ui.main_window import MainWindow
from arqstudiocad.ui.qt_services import QtUiServices


def ends(document: CadDocument, entity_id: int) -> list[tuple[float, float]]:
    """Where the outline of an entity runs, rounded, sorted."""
    bounds = document.get_entity(entity_id).bounds
    assert bounds is not None
    return [
        (round(bounds.min[0], 6), round(bounds.min[1], 6)),
        (round(bounds.max[0], 6), round(bounds.max[1], 6)),
    ]


def test_blocks_are_made_and_placed_through_the_api() -> None:
    doc = CadApplication().create_document("a")
    a, _ = doc.add_line((10, 10), (14, 10))
    b, _ = doc.add_line((10, 10), (10, 13))
    reference, made = doc.create_block("MARCO", (10, 10), [a, b])
    assert made.created == (reference,) and set(made.deleted) == {a, b}
    assert doc.undo_name == "BLOCK" and doc.block_names == ["MARCO"]
    assert ends(doc, reference) == [(10.0, 10.0), (14.0, 13.0)], "drawn where they were"
    placed, _ = doc.insert_block("MARCO", (0, 0), 2.0, math.pi / 2)
    assert ends(doc, placed) == [(-6.0, 0.0), (0.0, 8.0)], "twice as big, turned"
    with pytest.raises(InvalidOperationError, match="exists"):
        doc.create_block("marco", (0, 0), [placed])
    with pytest.raises(InvalidOperationError, match="no block named"):
        doc.insert_block("NADA", (0, 0))


class Harness:
    def __init__(self, ui: QuietUi | None = None) -> None:
        self.bus = EventBus()
        self.events: list[Event] = []
        self.bus.subscribe(Event, self.events.append)
        self.session = Session(self.bus)
        context = CommandContext(self.session, ui=ui)
        self.runner = CommandRunner(builtin_registry(), context)
        self.document = self.session.new_document()
        self.a, _ = self.document.add_line((10, 10), (14, 10))
        self.b, _ = self.document.add_line((10, 10), (10, 13))
        self.steps = len(self.document.undo_history)

    def run(self, *lines: str) -> None:
        for line in lines:
            self.runner.execute(line)

    def prompt(self) -> CommandPrompt:
        return [e for e in self.events if isinstance(e, CommandPrompt)][-1]

    def messages(self) -> list[str]:
        return [e.text for e in self.events if isinstance(e, CommandMessage)]

    def kinds(self) -> list[str]:
        return [e.kind for e in self.document.get_entities()]

    def new_steps(self) -> list[str]:
        return self.document.undo_history[self.steps :]


def test_block_turns_the_selection_into_a_block_reference() -> None:
    h = Harness()
    h.session.select(h.document, [h.a, h.b])
    h.run("BLOCK")
    assert h.prompt().text == "Block name:"
    h.run("MARCO", "10,10")
    assert h.kinds() == ["INSERT"] and h.document.block_names == ["MARCO"]
    assert h.messages()[-1] == "Block MARCO made from 2 objects."
    assert h.session.selection(h.document) == () and h.new_steps() == ["BLOCK"]


def test_block_asks_for_objects_and_refuses_names_in_use_or_invalid() -> None:
    h = Harness()
    h.run("B", "MARCO", "10,10")
    assert h.prompt().text.startswith("Select objects")
    h.session.select(h.document, [h.a])
    h.run("")
    assert h.document.block_names == ["MARCO"]
    h.run("BLOCK", "marco")
    assert h.messages()[-1] == "A block named marco exists in the drawing."
    assert h.prompt().text == "Block name:", "asked again"
    h.runner.cancel()
    h.session.select(h.document, [h.b])
    h.run("BLOCK", "A/B", "0,0")
    assert h.messages()[-1] == "A/B is not a valid block name."
    assert h.document.block_names == ["MARCO"]


def test_insert_follows_the_cursor_and_takes_scale_and_rotation() -> None:
    h = Harness()
    h.document.create_block("MARCO", (10, 10), [h.a, h.b])
    steps = len(h.document.undo_history)
    h.run("INSERT", "MARCO")
    prompt = h.prompt()
    assert prompt.text == "Specify insertion point [Scale(S)/Rotate(R)]:"
    assert prompt.drag is not None and prompt.drag.kind == "move"
    assert prompt.drag.band is False, "no rubber band from the origin"
    h.run("S", "2", "R", "90", "5,5")
    references = [e for e in h.document.get_entities() if e.kind == "INSERT"]
    assert len(references) == 2
    assert ends(h.document, references[-1].id) == [(-1.0, 5.0), (5.0, 13.0)]
    assert h.document.undo_history[steps:] == ["INSERT"], "one undo step"


def test_cancelling_insert_leaves_nothing() -> None:
    h = Harness()
    h.document.create_block("MARCO", (10, 10), [h.a, h.b])
    count, steps = len(h.document.get_entities()), len(h.document.undo_history)
    h.run("INSERT", "MARCO", "S", "3")
    h.runner.cancel()
    assert len(h.document.get_entities()) == count
    assert len(h.document.undo_history) == steps
    h.run("I", "NADA")
    assert h.messages()[-1] == "There is no such block."


def test_insert_chooses_the_block_in_a_window() -> None:
    ui = QuietUi(block_answer="MARCO")
    h = Harness(ui)
    h.run("INSERT")
    assert h.messages()[-1] == "The drawing has no blocks to insert."
    h.document.create_block("MARCO", (10, 10), [h.a, h.b])
    h.run("INSERT", "0,0")
    assert ui.asked[-1] == "Insert block: MARCO"
    assert [e.kind for e in h.document.get_entities()] == ["INSERT", "INSERT"]
    ui.block_answer = None
    h.run("INSERT")
    assert h.runner.active is None and len(h.document.get_entities()) == 2


def test_block_commands_in_spanish() -> None:
    h = Harness()
    set_language("es")
    for typed, english in (("BLOQUE", "BLOCK"), ("INSERTAR", "INSERT")):
        found = h.runner.registry.find(typed)
        assert found is not None and found.name == english


@pytest.fixture
def window(qtbot: QtBot) -> MainWindow:
    w = MainWindow(ui=QuietUi())
    qtbot.addWidget(w)
    return w


def test_blocks_are_in_the_insert_tab_menus_and_toolbar(window: MainWindow) -> None:
    from PySide6.QtWidgets import QMenu, QToolBar

    from arqstudiocad.ui.ribbon import Ribbon, RibbonGroupWidget

    insert, block = window.command_actions["INSERT"], window.command_actions["BLOCK"]
    assert (insert.text(), block.text()) == ("&Insert Block…", "Create &Block…")
    ribbon = window.findChildren(Ribbon)[0]
    tabs = [ribbon.tabText(i) for i in range(ribbon.count()) if ribbon.isTabVisible(i)]
    assert tabs == ["Home", "Insert", "Annotate", "View"]
    groups = {g.group.name: g for g in window.findChildren(RibbonGroupWidget)}
    bedit = window.command_actions["BEDIT"]
    assert [b.defaultAction() for b in groups["block"].buttons] == [insert, block, bedit]
    menus = {m.title(): m for m in window.menuBar().findChildren(QMenu)}
    assert insert in menus["&Insert"].actions() and block in menus["&Draw"].actions()
    for action in (insert, block):
        assert any(action in bar.actions() for bar in window.findChildren(QToolBar))


def test_a_drag_without_band_draws_no_rubber_band(window: MainWindow, tmp_path: Path) -> None:
    from PySide6.QtCore import QEvent
    from test_point_input import mouse, plan

    from arqstudiocad.events import Drag

    viewport = plan(window, tmp_path)  # a line (0,0)-(100,0), a circle at (50,40)
    window.session.drafting.set_osnap(False)

    def band_pixels(drag: Drag) -> int:
        """Pixels lit along the empty strip from (10,20) to (35,20)."""
        viewport.ask_point((10.0, 20.0), drag)
        mouse(viewport, QEvent.Type.MouseMove, (35.0, 20.0))
        image = viewport.grab().toImage()
        ratio = viewport.devicePixelRatioF()
        (x0, y0), (x1, _) = viewport.view.to_screen(10.0, 20.0), viewport.view.to_screen(35.0, 20.0)
        y = round(y0 / ratio)
        lit = 0
        for x in range(round(x0 / ratio) + 4, round(x1 / ratio) - 10):
            clear = image.pixelColor(x, y - 6).lightness()
            rows = (image.pixelColor(x, y + dy).lightness() for dy in (-1, 0))
            lit += any(abs(r - clear) > 15 for r in rows)
        viewport.end_point()
        return lit

    document = window.session.active
    assert document is not None
    shown = document.get_entities()[0].id
    assert band_pixels(Drag("move", (shown,))) > 10, "a band joins the base to the cursor"
    assert band_pixels(Drag("move", (shown,), band=False)) == 0


# The template's names (ADR 0025 D6, step 7c)


def test_block_warns_of_a_name_the_template_uses_and_goes_on() -> None:
    h = Harness()
    h.session.template = h.session.app.open_document(sample_template())
    h.run("BLOCK", "wc")
    assert h.messages()[-1] == (
        "The template arqstudiocad-sample.dwt has a block named WC: inserting it "
        "from the template will ask what to do."
    )
    assert h.prompt().text.startswith("Specify base point"), "not refused: two files"
    h.run("10,10")
    h.session.select(h.document, [h.a])
    h.run("")
    assert h.document.block_names == ["wc"]


def clashing(h: Harness) -> CadDocument:
    """The harness drawing with its own wc (one line), and the sample
    template, whose WC differs."""
    reference, _ = h.document.create_block("wc", (10, 10), [h.a])
    h.document.delete_entities([reference])
    return h.session.app.open_document(sample_template())


def test_insert_asks_about_a_clash_at_the_command_line() -> None:
    h = Harness()
    template = clashing(h)
    steps = len(h.document.undo_history)
    h.runner.start(insert_named("WC", 1.0, 0.0, False, source=template))
    assert h.prompt().text.startswith("The drawing has another block named wc")
    h.run("C")
    assert h.prompt().text.startswith("New block name") and "WC_2" in h.prompt().text
    h.run("")
    assert sorted(h.document.block_names, key=str.lower) == ["wc", "WC_2"]
    assert len(h.document.undo_history) == steps + 1 and h.document.undo_name == "INSERT"
    h.runner.start(insert_named("WC", 1.0, 0.0, False, source=template))
    h.run("R")
    assert h.document.block_clashes("WC", template) == [], "redefined from the template"
    h.runner.start(insert_named("WC", 1.0, 0.0, False, source=template))
    assert h.runner.active is None, "nothing left to ask: the same block now"


def test_a_point_given_first_places_the_block_there_without_asking() -> None:
    h = Harness()
    template = h.session.app.open_document(sample_template())
    h.runner.start(insert_named("Tree", 1.0, 0.0, True, source=template, at=(50.0, 60.0)))
    assert h.runner.active is None, "dropped first: no point is asked"
    (placed,) = [e.id for e in h.document.get_entities() if e.kind == "INSERT"]
    (x0, y0), (x1, y1) = ends(h.document, placed)
    assert x0 < 50 < x1 and y0 < 60 < y1, "around the point given"


def used(h: Harness) -> list[BlockUsed]:
    return [e for e in h.events if isinstance(e, BlockUsed)]


def test_block_and_insert_say_which_block_was_used(tmp_path: Path) -> None:
    """For the Blocks palette's Recent source (ADR 0025 D1)."""
    h = Harness()
    h.session.select(h.document, [h.a])
    h.run("BLOCK", "MARCO", "10,10")
    assert used(h) == [BlockUsed(h.document.id, "MARCO", None, "created")]
    h.run("INSERT", "marco", "S", "2")
    h.runner.cancel()
    assert len(used(h)) == 1, "cancelled: nothing was inserted"
    h.run("INSERT", "marco", "5,5")
    assert used(h)[-1] == BlockUsed(h.document.id, "MARCO", None, "inserted"), (
        "as the drawing spells it"
    )
    h.runner.start(insert_named("MARCO", 1.0, 0.0, False))
    assert len(used(h)) == 3
    template = h.session.app.open_document(sample_template())
    h.runner.start(insert_named("Tree", 1.0, 0.0, False, source=template))
    assert used(h)[-1] == BlockUsed(template.id, "Tree", sample_template(), "inserted")
    h.session.template = template
    h.runner.start(insert_named("Tree", 1.0, 0.0, True, source=template))
    h.run("0,0")
    assert used(h)[-1] == BlockUsed(template.id, "Tree", sample_template(), "template")


def test_insert_reads_a_recent_blocks_file_first(tmp_path: Path) -> None:
    h = Harness()
    loaded: list[CadDocument] = []
    steps = len(h.document.undo_history)
    h.runner.start(
        insert_named("Tree", 1.0, 0.0, False, load=sample_template(), loaded=loaded.append)
    )
    (read,) = loaded
    assert read.path == sample_template() and read not in h.session.documents, "never a tab"
    assert "Tree" in h.document.block_names and h.document.undo_history[steps:] == ["INSERT"]
    assert used(h)[-1] == BlockUsed(read.id, "Tree", sample_template(), "inserted")
    missing = tmp_path / "gone.dwg"
    h.runner.start(insert_named("Tree", 1.0, 0.0, False, load=missing, loaded=loaded.append))
    assert h.messages()[-1].startswith("gone.dwg cannot be read: ")
    assert len(loaded) == 1 and len(h.document.undo_history) == steps + 1


def test_block_says_a_name_in_use_in_a_dialog_too() -> None:
    """The maintainer's request (2026-10-25): with an interface, BLOCK's
    refusal and the template's warning are dialogs as well."""
    ui = QuietUi(template_name_answer=False)
    h = Harness(ui)
    h.document.create_block("MARCO", (10, 10), [h.a])
    h.session.template = h.session.app.open_document(sample_template())
    h.run("BLOCK", "marco")
    assert ui.asked[-1] == "name taken MARCO" and h.prompt().text == "Block name:"
    h.run("wc")
    assert ui.asked[-1] == "name in template WC arqstudiocad-sample.dwt"
    assert h.prompt().text == "Block name:", "Another name: asked again"
    ui.template_name_answer = True
    h.run("wc")
    assert h.prompt().text.startswith("Specify base point"), "Continue: BLOCK goes on"
    h.run("0,0")
    h.session.select(h.document, [h.b])
    h.run("")
    assert sorted(h.document.block_names) == ["MARCO", "wc"]


def answer_box(
    monkeypatch: pytest.MonkeyPatch, button: str | QMessageBox.StandardButton
) -> list[str]:
    """The next message box shown is answered with ``button``, a standard
    button (its text varies with the platform's style) or one of ours by
    its text; the box's text."""
    seen: list[str] = []

    def answer(box: QMessageBox) -> int:
        seen.append(box.text())
        if isinstance(button, QMessageBox.StandardButton):
            chosen = box.button(button)
        else:
            (chosen,) = [b for b in box.buttons() if b.text() == button]
        assert chosen is not None, "the box has the button"
        chosen.click()
        return 0

    monkeypatch.setattr(QMessageBox, "exec", answer)
    return seen


def test_the_block_name_dialogs(qtbot: QtBot, monkeypatch: pytest.MonkeyPatch) -> None:
    parent = QWidget()
    qtbot.addWidget(parent)
    services = QtUiServices(parent)
    seen = answer_box(monkeypatch, QMessageBox.StandardButton.Ok)
    services.report_block_name_taken("MARCO")
    assert seen == ["A block named MARCO exists in the drawing. Type another name."]
    seen = answer_box(monkeypatch, "Continue")
    assert services.ask_block_name_in_template("WC", "plantilla.dwt")
    assert seen[0].startswith("The template plantilla.dwt has a block named WC.")
    answer_box(monkeypatch, "Another name")
    assert not services.ask_block_name_in_template("WC", "plantilla.dwt")
