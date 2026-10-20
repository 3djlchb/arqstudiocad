"""Basic multiline text (ADR 0022 D6): the core API with word wrapping,
the MTEXT command and editing an MTEXT's paragraphs in Properties."""

import math

import pytest
from pytestqt.qtbot import QtBot
from test_panels import property_item
from ui_fakes import QuietUi

from arqstudiocad.commands import CommandContext, CommandRunner
from arqstudiocad.commands.builtin import builtin_registry
from arqstudiocad.core import CadApplication, CadDocument, CadText, InvalidOperationError
from arqstudiocad.events import CommandMessage, CommandPrompt, Event, EventBus
from arqstudiocad.i18n import set_language
from arqstudiocad.session import Session
from arqstudiocad.ui import text_dialog
from arqstudiocad.ui.main_window import MainWindow
from arqstudiocad.ui.property_grid import EDIT_IN_DIALOG


def text_of(document: CadDocument, entity_id: int) -> CadText:
    text = document.get_entity(entity_id).text
    assert text is not None
    return text


def test_multiline_text_is_written_wrapped_and_read_as_lines() -> None:
    doc = CadApplication().create_document("a")
    a, made = doc.add_mtext("Uno dos tres\nCuatro {5}", (1, 2), 2.0, 0.0, 0.0)
    assert made.created == (a,) and doc.undo_name == "MTEXT"
    text = text_of(doc, a)
    assert (text.content, text.justification, text.width) == (
        "Uno dos tres\nCuatro {5}",
        "TopLeft",
        0.0,
    )
    assert text.is_multiline and text.content_editable
    assert doc.get_entity(a).kind == "MTEXT"
    unwrapped = doc.get_entity(a).bounds
    b, _ = doc.add_mtext("Uno dos tres\nCuatro {5}", (1, 2), 2.0, 0.0, 6.0, "BottomRight")
    wrapped = doc.get_entity(b).bounds
    assert unwrapped is not None and wrapped is not None
    assert wrapped.height > unwrapped.height, "wrapped to more lines"
    assert wrapped.width < unwrapped.width
    with pytest.raises(InvalidOperationError, match="justification"):
        doc.add_mtext("x", (0, 0), 2.0, 0.0, 0.0, "Sideways")


class Harness:
    def __init__(self, ui: QuietUi | None = None) -> None:
        self.bus = EventBus()
        self.events: list[Event] = []
        self.bus.subscribe(Event, self.events.append)
        self.session = Session(self.bus)
        context = CommandContext(self.session, ui=ui)
        self.runner = CommandRunner(builtin_registry(), context)
        self.document = self.session.new_document()

    def run(self, *lines: str) -> None:
        for line in lines:
            self.runner.execute(line)

    def messages(self) -> list[str]:
        return [e.text for e in self.events if isinstance(e, CommandMessage)]

    def prompt(self) -> str:
        return [e.text for e in self.events if isinstance(e, CommandPrompt)][-1]

    def texts(self) -> list[CadText]:
        return [e.text for e in self.document.get_entities() if e.text is not None]


def test_mtext_takes_a_rectangle_and_lines_typed() -> None:
    h = Harness()
    h.run("MTEXT")
    assert h.prompt() == "Specify first corner:"
    h.run("0,0")
    assert h.prompt() == "Specify opposite corner [Height(H)/Justify(J)/Rotation(R)]:"
    h.run("20,-10", "Hola", "mundo", "")
    (text,) = h.texts()
    assert (text.content, text.width, text.justification) == ("Hola\nmundo", 20.0, "TopLeft")
    assert text.anchor == pytest.approx((0.0, 0.0))
    assert h.document.undo_history == ["MTEXT"]


def test_mtext_options_set_height_justification_and_rotation() -> None:
    h = Harness()
    h.run("T", "0,0", "H", "5", "J")
    assert h.prompt().startswith("Enter justification [Top left(TL)/Top center(TC)")
    h.run("MC", "R", "90")
    # Rotated 90 degrees: reading upwards, the rectangle 20 long and 10 deep.
    h.run("-10,20", "Centro", "")
    (text,) = h.texts()
    assert (text.height, text.justification, text.width) == (5.0, "MiddleCenter", 20.0)
    assert text.rotation == pytest.approx(math.pi / 2)
    assert text.anchor == pytest.approx((-5.0, 10.0)), "the middle of the rectangle"
    h.run("MT", "0,0")
    assert h.prompt() == "Specify opposite corner [Height(H)/Justify(J)/Rotation(R)]:"
    h.run("J")
    assert h.prompt().endswith("<MC>:"), "the last justification"


def test_mtext_takes_its_text_from_the_window_and_cancelling_draws_nothing() -> None:
    ui = QuietUi(text_answer="Línea 1\nLínea 2")
    h = Harness(ui)
    h.run("MTEXT", "0,0", "30,-10")
    assert ui.asked[-1] == "text Multiline text: "
    assert [t.content for t in h.texts()] == ["Línea 1\nLínea 2"]
    ui.text_answer = None
    h.run("MTEXT", "0,0", "30,-10")
    ui.text_answer = "   "
    h.run("MTEXT", "0,0", "30,-10")
    assert len(h.texts()) == 1 and h.document.undo_history == ["MTEXT"]
    assert not any("Error" in m for m in h.messages()), "nothing typed is no error"


def test_mtext_in_spanish() -> None:
    h = Harness()
    set_language("es")
    h.run("TEXTOM")
    assert h.runner.active == "MTEXT"
    h.run("0,0")
    assert h.prompt() == "Precise esquina opuesta [Altura(H)/Justificar(J)/Rotación(R)]:"


@pytest.fixture
def window(qtbot: QtBot) -> MainWindow:
    w = MainWindow(ui=QuietUi())
    qtbot.addWidget(w)
    return w


def test_properties_edit_an_mtexts_paragraphs_in_the_window(
    window: MainWindow, monkeypatch: pytest.MonkeyPatch
) -> None:
    window.runner.execute("NEW")
    document = window.session.active
    assert document is not None
    a, changes = document.add_mtext("Uno\nDos", (0, 0), 2.0, 0.0, 25.0)
    window.session.report(document, changes)
    window.session.select(document, [a])
    panel = window.properties_panel
    assert panel.grid.caption.text() == "Multiline text"
    rows = dict(panel.rows())
    assert (rows["Contents"], rows["Width"]) == ("Uno ¶ Dos", "25")
    asked: list[str] = []

    def answer(parent: object, title: str, text: str = "") -> str:
        asked.append(text)
        return "Tres\nCuatro"

    monkeypatch.setattr(text_dialog, "ask_text", answer)
    panel.grid.property_edited.emit("text.content", EDIT_IN_DIALOG)
    assert asked == ["Uno\nDos"], "the window opens with its paragraphs"
    assert text_of(document, a).content == "Tres\nCuatro"
    assert document.undo_history[-1] == "PROPERTIES"
    tree = panel.grid.tree
    tree.itemDoubleClicked.emit(property_item(window, "Contents"), 1)
    assert len(asked) == 2, "a double click opens it too"


def test_the_rectangle_may_be_drawn_in_any_direction() -> None:
    h = Harness()
    # From its top right corner to its bottom left: the text still hangs
    # from the top left corner.
    h.run("MTEXT", "20,0", "0,-10", "A", "")
    (text,) = h.texts()
    assert text.anchor == pytest.approx((0.0, 0.0)) and text.width == 20.0
