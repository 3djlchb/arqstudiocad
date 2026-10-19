"""TEXT and MTEXT (ADR 0022): the core API, editing commands and grips on
texts, the TEXT command and the Text section of Properties."""

import math
from pathlib import Path

import pytest
from PySide6.QtCore import Qt
from pytestqt.qtbot import QtBot
from test_panels import property_item
from ui_fakes import QuietUi

from arqstudiocad.commands import CommandContext, CommandRunner
from arqstudiocad.commands.builtin import builtin_registry
from arqstudiocad.commands.grips import grip_edit, shown_grips
from arqstudiocad.core import CadApplication, CadDocument, CadText, InvalidOperationError
from arqstudiocad.events import CommandPrompt, Event, EventBus
from arqstudiocad.i18n import set_language
from arqstudiocad.session import Session
from arqstudiocad.ui.main_window import MainWindow


def text_of(document: CadDocument, entity_id: int) -> CadText:
    text = document.get_entity(entity_id).text
    assert text is not None
    return text


def mtexts(tmp_path: Path) -> CadDocument:
    """An MTEXT with formatting codes and a plain one."""
    codes = ["0", "SECTION", "2", "ENTITIES"]
    for content in ("{\\fArial;A}\\PB", "Plain"):
        codes += ["0", "MTEXT", "8", "0", "10", "1", "20", "1", "30", "0", "40", "2.5"]
        codes += ["71", "1", "1", content]
    codes += ["0", "ENDSEC", "0", "EOF", ""]
    path = tmp_path / "mtext.dxf"
    path.write_text("\n".join(codes), encoding="ascii")
    return CadApplication().open_document(path)


# The core API


def test_texts_are_written_read_and_edited_all_or_nothing() -> None:
    doc = CadApplication().create_document("a")
    assert doc.text_size == pytest.approx(2.5)
    a, made = doc.add_text("Hola", (1, 2), 2.0, math.pi / 6)
    assert made.created == (a,) and doc.undo_name == "TEXT"
    text = text_of(doc, a)
    assert (text.content, text.anchor, text.height) == ("Hola", (1.0, 2.0), 2.0)
    assert text.rotation == pytest.approx(math.pi / 6)
    assert (text.style, text.justification, text.is_multiline) == ("Standard", "Left", False)
    assert text.sizes_editable and text.content_editable
    entity = doc.get_entity(a)
    assert (entity.kind, entity.is_transformable, entity.geometry) == ("TEXT", True, None)

    b, _ = doc.add_text("Mundo", (1, 0), 2.0, 0.0)
    doc.set_text([a, b], height=3.0)
    assert text_of(doc, b).height == 3.0 and doc.undo_name == "PROPERTIES"
    line, _ = doc.add_line((0, 0), (1, 0))
    with pytest.raises(InvalidOperationError):
        doc.set_text([a, line], content="X")
    assert text_of(doc, a).content == "Hola", "nothing changed"
    with pytest.raises(InvalidOperationError, match="one line"):
        doc.set_text([a], content="a\nb")
    with pytest.raises(InvalidOperationError, match="greater than zero"):
        doc.add_text("x", (0, 0), 0.0, 0.0)
    assert [g.kind for g in doc.grips([a])] == ["INS"]


def test_an_mtext_with_formatting_keeps_its_content(tmp_path: Path) -> None:
    doc = mtexts(tmp_path)
    formatted, plain = (e.id for e in doc.get_entities())
    assert not text_of(doc, formatted).content_editable
    assert text_of(doc, plain).content_editable and text_of(doc, plain).is_multiline
    with pytest.raises(InvalidOperationError, match="formatting"):
        doc.set_text([formatted], content="X")
    doc.set_text([plain], content="Otro", rotation=math.pi / 2)
    assert text_of(doc, plain).content == "Otro"


# Commands and grips


class Harness:
    def __init__(self) -> None:
        self.bus = EventBus()
        self.events: list[Event] = []
        self.bus.subscribe(Event, self.events.append)
        self.session = Session(self.bus)
        self.runner = CommandRunner(builtin_registry(), CommandContext(self.session))
        self.document = self.session.new_document()

    def run(self, *lines: str) -> None:
        for line in lines:
            self.runner.execute(line)

    def prompt(self) -> str:
        return [e.text for e in self.events if isinstance(e, CommandPrompt)][-1]

    def texts(self) -> list[CadText]:
        return [e.text for e in self.document.get_entities() if e.text is not None]


def test_editing_commands_move_rotate_and_mirror_texts() -> None:
    h = Harness()
    a, _ = h.document.add_text("A", (2, 3), 1.0, 0.0)
    h.session.select(h.document, [a])
    h.run("MOVE", "0,0", "1,0")
    assert text_of(h.document, a).anchor == pytest.approx((3.0, 3.0))
    h.session.select(h.document, [a])
    h.run("ROTATE", "0,0", "90")
    assert text_of(h.document, a).anchor == pytest.approx((-3.0, 3.0))
    assert text_of(h.document, a).rotation == pytest.approx(math.pi / 2)
    h.session.select(h.document, [a])
    h.run("ROTATE", "0,0", "-90")
    h.session.select(h.document, [a])
    h.run("MIRROR", "0,0", "0,1", "Y")
    mirrored = text_of(h.document, a)
    assert mirrored.justification == "Right", "readable, as AutoCAD's MIRRTEXT 0"
    assert mirrored.anchor == pytest.approx((-3.0, 3.0)), "the anchor mirrored from (3, 3)"
    assert math.cos(mirrored.rotation) == pytest.approx(1.0)
    h.session.select(h.document, [a])
    h.run("COPY", "0,0", "0,10", "")
    assert [t.content for t in h.texts()] == ["A", "A"]


def test_a_texts_grip_moves_it() -> None:
    h = Harness()
    a, _ = h.document.add_text("A", (2, 3), 1.0, 0.0)
    h.session.select(h.document, [a])
    (grip,) = shown_grips(h.session, h.document)
    assert (grip.kind, grip.point) == ("INS", (2.0, 3.0))
    h.runner.start(grip_edit(grip))
    h.run("5,5")
    assert text_of(h.document, a).anchor == pytest.approx((5.0, 5.0))


def test_the_text_command_writes_one_text_per_line() -> None:
    h = Harness()
    h.run("TEXT")
    assert h.prompt() == "Specify start point of text:"
    h.run("0,0")
    assert h.prompt() == "Specify height <2.5>:"
    h.run("3")
    assert h.prompt() == "Specify rotation angle of text <0>:"
    h.run("")
    assert h.prompt() == "Enter text:"
    h.run("Hola mundo", "Segunda", "")
    assert h.runner.active is None
    first, second = h.texts()
    assert (first.content, first.anchor, first.height) == ("Hola mundo", (0.0, 0.0), 3.0)
    assert second.content == "Segunda"
    assert second.anchor == pytest.approx((0.0, -5.0)), "5/3 of a height below"
    assert h.document.undo_history[-1] == "TEXT" and len(h.document.undo_history) == 1
    h.run("DT", "10,0")
    assert h.prompt() == "Specify height <3>:", "the last height"
    h.run("", "90", "Vertical", "Next", "")
    third, fourth = h.texts()[2:]
    assert third.rotation == pytest.approx(math.pi / 2)
    assert fourth.anchor == pytest.approx((15.0, 0.0)), "below, across the reading direction"
    h.run("TEXT", "0,0", "", "")
    h.runner.cancel()
    assert len(h.texts()) == 4 and len(h.document.undo_history) == 2, "nothing typed, no step"


def test_the_text_command_in_spanish() -> None:
    h = Harness()
    set_language("es")
    h.run("TEXTO")
    assert h.runner.active == "TEXT"
    assert h.prompt() == "Precise punto inicial del texto:"


# Properties


@pytest.fixture
def window(qtbot: QtBot) -> MainWindow:
    ui = QuietUi()
    w = MainWindow(ui=ui)
    qtbot.addWidget(w)
    return w


def test_properties_show_and_edit_a_text(window: MainWindow) -> None:
    window.runner.execute("NEW")
    document = window.session.active
    assert document is not None
    a, changes = document.add_text("Hola", (1, 2), 2.0, 0.0)
    window.session.report(document, changes)
    window.session.select(document, [a])
    panel = window.properties_panel
    assert panel.grid.caption.text() == "Text"
    rows = dict(panel.rows())
    assert (rows["Contents"], rows["Height"], rows["Rotation"]) == ("Hola", "2", "0")
    assert (rows["Style"], rows["Justification"], rows["Width factor"]) == ("Standard", "Left", "1")
    assert property_item(window, "Contents").flags() & Qt.ItemFlag.ItemIsEditable

    property_item(window, "Contents").setText(1, "Adiós")
    assert text_of(document, a).content == "Adiós"
    assert document.undo_history[-1] == "PROPERTIES"
    property_item(window, "Height").setText(1, "5")
    property_item(window, "Rotation").setText(1, "90")
    text = text_of(document, a)
    assert text.height == 5.0 and text.rotation == pytest.approx(math.pi / 2)
    property_item(window, "Height").setText(1, "0")
    assert text_of(document, a).height == 5.0, "refused and reported"
    assert dict(panel.rows())["Height"] == "5"


def test_properties_keep_formatted_mtext_read_only(window: MainWindow, tmp_path: Path) -> None:
    codes = ["0", "SECTION", "2", "ENTITIES"]
    codes += ["0", "MTEXT", "8", "0", "10", "1", "20", "1", "30", "0", "40", "2.5"]
    codes += ["71", "1", "1", "{\\fArial;A}", "0", "ENDSEC", "0", "EOF", ""]
    path = tmp_path / "mtext.dxf"
    path.write_text("\n".join(codes), encoding="ascii")
    document = window.session.open_document(path)
    window.session.select(document, [document.get_entities()[0].id])
    assert window.properties_panel.grid.caption.text() == "Multiline text"
    assert not property_item(window, "Contents").flags() & Qt.ItemFlag.ItemIsEditable
    assert property_item(window, "Height").flags() & Qt.ItemFlag.ItemIsEditable
    assert dict(window.properties_panel.rows())["Justification"] == "Top left"


def test_text_is_in_the_draw_menu_ribbon_and_toolbar(window: MainWindow) -> None:
    from PySide6.QtWidgets import QMenu, QToolBar

    from arqstudiocad.ui.ribbon import RibbonGroupWidget

    action = window.command_actions["TEXT"]
    assert action.text() == "&Text"
    assert any(action in menu.actions() for menu in window.menuBar().findChildren(QMenu))
    groups = {g.group.name: g for g in window.findChildren(RibbonGroupWidget)}
    assert action in [b.defaultAction() for b in groups["draw"].buttons]
    assert any(action in bar.actions() for bar in window.findChildren(QToolBar))
