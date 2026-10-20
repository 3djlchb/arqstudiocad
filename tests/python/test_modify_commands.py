"""ERASE, MOVE, COPY, ROTATE, SCALE and MIRROR (ADR 0016 D3, D8): selecting
first or at "Select objects", locked layers, and the drag preview."""

from pathlib import Path

import pytest
from PySide6.QtCore import QEvent, QPointF, Qt
from PySide6.QtGui import QKeyEvent, QKeySequence
from PySide6.QtWidgets import QApplication
from pytestqt.qtbot import QtBot
from test_point_input import mouse, plan
from ui_fakes import QuietUi

from arqstudiocad.commands import CommandContext, CommandRunner
from arqstudiocad.commands.builtin import builtin_registry
from arqstudiocad.core import CadDocument, CadLine
from arqstudiocad.events import (
    CommandMessage,
    CommandPrompt,
    DocumentChanged,
    Drag,
    Event,
    EventBus,
)
from arqstudiocad.session import Session
from arqstudiocad.ui.main_window import MainWindow


class Harness:
    """A drawing with two lines: (0,0)-(10,0) and (0,5)-(10,5)."""

    def __init__(self) -> None:
        self.bus = EventBus()
        self.events: list[Event] = []
        self.bus.subscribe(Event, self.events.append)
        self.session = Session(self.bus)
        self.runner = CommandRunner(builtin_registry(), CommandContext(self.session))
        self.document: CadDocument = self.session.new_document()
        self.a, _ = self.document.add_line((0.0, 0.0), (10.0, 0.0))
        self.b, _ = self.document.add_line((0.0, 5.0), (10.0, 5.0))

    def run(self, *lines: str) -> None:
        for line in lines:
            self.runner.execute(line)

    def select(self, *ids: int) -> None:
        self.session.select(self.document, ids)

    def selection(self) -> tuple[int, ...]:
        return self.session.selection(self.document)

    def messages(self) -> list[str]:
        return [e.text for e in self.events if isinstance(e, CommandMessage)]

    def prompt(self) -> CommandPrompt:
        return [e for e in self.events if isinstance(e, CommandPrompt)][-1]

    def line(self, entity_id: int) -> tuple[tuple[float, float], tuple[float, float]]:
        geometry = self.document.get_entity(entity_id).geometry
        assert isinstance(geometry, CadLine)
        return geometry.start, geometry.end

    def flat(self, entity_id: int) -> tuple[float, ...]:
        """A line's four coordinates, for pytest.approx."""
        (x1, y1), (x2, y2) = self.line(entity_id)
        return (x1, y1, x2, y2)

    def ids(self) -> list[int]:
        return [e.id for e in self.document.get_entities()]


# Selecting


def test_erase_takes_the_selection_first_and_empties_it() -> None:
    h = Harness()
    h.select(h.a)
    h.run("ERASE")
    assert h.ids() == [h.b] and h.selection() == ()
    assert h.runner.active is None, "noun-verb asks nothing"
    h.run("UNDO")
    assert h.ids() == [h.a, h.b]


def test_select_objects_takes_clicks_until_enter() -> None:
    h = Harness()
    h.run("E")
    assert h.prompt().text == "Select objects [All/Last(L)/Previous(P)]:"
    assert not h.prompt().point, "the viewport selects, it does not pick points"
    h.select(h.b)  # what a click in the viewport does
    h.run("xyz")
    assert h.messages()[-1] == "Select objects in the drawing, or type an option."
    h.run("")
    assert h.ids() == [h.a]


def test_all_last_and_previous_add_to_the_selection() -> None:
    h = Harness()
    h.run("ERASE", "L")
    assert h.selection() == (h.b,) and h.messages()[-1] == "1 found"
    h.run("ALL")
    assert set(h.selection()) == {h.a, h.b} and h.messages()[-1] == "2 found"
    h.runner.cancel()
    h.session.clear_selection(h.document)
    h.run("MOVE", "ALL", "", "0,0", "1,0")
    assert h.line(h.a) == ((1.0, 0.0), (11.0, 0.0))
    h.run("MOVE", "P")
    assert set(h.selection()) == {h.a, h.b}, "the previous command's objects"
    h.run("", "0,0", "0,1")
    assert h.line(h.b) == ((1.0, 6.0), (11.0, 6.0))


def test_enter_with_nothing_selected_ends_quietly() -> None:
    h = Harness()
    before = h.document.undo_history
    h.run("MOVE", "")
    assert h.runner.active is None and h.document.undo_history == before


def test_objects_on_locked_layers_are_left_out() -> None:
    h = Harness()
    layer_id, _ = h.document.create_layer("Locked")
    h.document.set_entity_layer([h.b], layer_id)
    h.document.set_layer_locked(layer_id, True)
    h.select(h.a, h.b)
    h.run("MOVE")
    assert h.messages()[-1] == "1 were on a locked layer."
    assert h.selection() == (h.a,)
    h.run("0,0", "0,2")
    assert h.line(h.a) == ((0.0, 2.0), (10.0, 2.0)) and h.line(h.b) == ((0.0, 5.0), (10.0, 5.0))
    h.select(h.b)
    h.run("ERASE")
    assert h.runner.active is None and h.ids() == [h.a, h.b], "nothing left to erase"


def test_objects_that_cannot_be_transformed_yet_stop_the_command(tmp_path: Path) -> None:
    path = tmp_path / "point.dxf"
    codes = ["0", "SECTION", "2", "ENTITIES", "0", "POINT", "8", "0", "10", "1", "20", "1"]
    codes += ["30", "0", "0", "ENDSEC", "0", "EOF", ""]
    path.write_text("\n".join(codes), encoding="ascii")
    h = Harness()
    document = h.session.open_document(path)
    point_id = document.get_entities()[0].id
    h.session.select(document, [point_id])
    h.run("MOVE")
    assert h.messages()[-1] == "These objects cannot be transformed yet: POINT."
    assert h.runner.active is None
    h.run("ERASE")
    assert document.get_entities() == [], "erasing works on every kind"


def blocks(tmp_path: Path, attribute: bool = False) -> Path:
    """Block PUERTA, base (1,1), one LINE to (2,1); an INSERT of it at
    (10,5) and a LINE (0,0)-(1,0). With ``attribute``, the block defines
    NUM and the reference's NUM is "P-01" at (10,6)."""
    codes = ["0", "SECTION", "2", "BLOCKS"]
    codes += ["0", "BLOCK", "8", "0", "2", "PUERTA", "70", "0", "10", "1", "20", "1", "30", "0"]
    codes += ["3", "PUERTA", "0", "LINE", "8", "0", "10", "1", "20", "1", "30", "0"]
    codes += ["11", "2", "21", "1", "31", "0"]
    if attribute:
        codes += ["0", "ATTDEF", "8", "0", "10", "1", "20", "1", "30", "0", "40", "0.2"]
        codes += ["1", "", "2", "NUM", "3", "Number", "70", "0"]
    codes += ["0", "ENDBLK", "8", "0", "0", "ENDSEC"]
    codes += ["0", "SECTION", "2", "ENTITIES"]
    if attribute:
        codes += ["0", "INSERT", "8", "0", "66", "1", "2", "PUERTA", "10", "10", "20", "5"]
        codes += ["30", "0", "0", "ATTRIB", "8", "0", "10", "10", "20", "6", "30", "0"]
        codes += ["40", "0.2", "1", "P-01", "2", "NUM", "70", "0", "0", "SEQEND", "8", "0"]
    else:
        codes += ["0", "INSERT", "8", "0", "2", "PUERTA", "10", "10", "20", "5", "30", "0"]
    codes += ["0", "LINE", "8", "0", "10", "0", "20", "0", "30", "0", "11", "1", "21", "0"]
    codes += ["31", "0", "0", "ENDSEC", "0", "EOF", ""]
    path = tmp_path / "bloques.dxf"
    path.write_text("\n".join(codes), encoding="ascii")
    return path


def test_explode_replaces_a_block_reference_with_its_contents(tmp_path: Path) -> None:
    h = Harness()
    document = h.session.open_document(blocks(tmp_path))
    door, line = document.get_entities()
    h.session.select(document, [door.id])
    h.run("X")
    entities = document.get_entities()
    assert [e.kind for e in entities] == ["LINE", "LINE"]
    assert door.id not in [e.id for e in entities] and line.id in [e.id for e in entities]
    exploded = next(e for e in entities if e.id != line.id).geometry
    assert exploded == CadLine((10.0, 5.0), (11.0, 5.0)), "placed as the block was drawn"
    published = [e.changes for e in h.events if isinstance(e, DocumentChanged)]
    assert any(door.id in c.deleted and c.created for c in published), "the viewport learns"
    assert h.runner.active is None and h.session.selection(document) == ()
    assert document.undo_name == "EXPLODE"
    h.run("UNDO")
    assert [e.id for e in document.get_entities()] == [door.id, line.id]


def test_explode_refuses_what_is_not_a_block_and_keeps_attribute_values(tmp_path: Path) -> None:
    h = Harness()
    document = h.session.open_document(blocks(tmp_path))
    door, line = document.get_entities()
    h.session.select(document, [door.id, line.id])
    h.run("EXPLODE")
    assert h.messages()[-1] == "Only block references can be exploded."
    assert [e.id for e in document.get_entities()] == [door.id, line.id]
    with_attribute = h.session.open_document(blocks(tmp_path, attribute=True))
    door = with_attribute.get_entities()[0]
    h.session.select(with_attribute, [door.id])
    h.run("EXPLODE")
    # Its attribute's value stays, as a text where it was drawn (ADR 0023).
    texts = [e.text for e in with_attribute.get_entities() if e.text is not None]
    assert [(t.content, t.anchor) for t in texts] == [("P-01", (10.0, 6.0))]
    assert "ATTDEF" not in [e.kind for e in with_attribute.get_entities()]


# The commands


def test_move_from_a_base_to_a_second_point_with_a_drag() -> None:
    h = Harness()
    h.select(h.a)
    h.run("M", "1,1")
    prompt = h.prompt()
    assert prompt.text == "Specify second point <use first point as displacement>:"
    assert prompt.base == (1.0, 1.0) and prompt.drag == Drag("move", (h.a,))
    h.run("@2,3")
    assert h.line(h.a) == ((2.0, 3.0), (12.0, 3.0))
    assert h.selection() == () and h.document.undo_history[-1] == "MOVE"
    h.select(h.a)
    h.run("MOVE", "1,1", "")
    assert h.line(h.a) == ((3.0, 4.0), (13.0, 4.0)), "Enter: the base point is the displacement"


def test_copy_makes_as_many_copies_as_points_in_one_undo_step() -> None:
    h = Harness()
    h.select(h.a)
    h.run("CP", "0,0", "0,10", "0,20", "0,30", "U", "")
    ids = h.ids()
    assert len(ids) == 4, "two lines and two copies; the third was undone"
    assert h.line(ids[2]) == ((0.0, 10.0), (10.0, 10.0))
    assert h.line(ids[3]) == ((0.0, 20.0), (10.0, 20.0))
    assert h.line(h.a) == ((0.0, 0.0), (10.0, 0.0)), "the source stays"
    assert h.document.undo_history[-1] == "COPY"
    h.run("UNDO")
    assert h.ids() == [h.a, h.b]


def test_rotate_by_a_typed_or_shown_angle_remembers_it() -> None:
    h = Harness()
    h.select(h.a)
    h.run("RO", "0,0")
    assert h.prompt().text == "Specify rotation angle:"
    assert h.prompt().drag == Drag("rotate", (h.a,))
    h.run("90")
    start, end = h.line(h.a)
    assert start == pytest.approx((0.0, 0.0)) and end == pytest.approx((0.0, 10.0), abs=1e-9)
    h.select(h.a)
    h.run("ROTATE", "0,0")
    assert h.prompt().text == "Specify rotation angle <90>:"
    h.runner.answer_point((0.0, 0.0))
    assert h.messages()[-1] == "A point on the base point shows no direction."
    h.runner.answer_point((-3.0, 0.0))  # 180 degrees
    assert h.line(h.a)[1] == pytest.approx((0.0, -10.0), abs=1e-9)


def test_scale_by_a_factor_or_a_distance() -> None:
    h = Harness()
    h.select(h.a)
    h.run("SC", "0,0", "0")
    assert h.messages()[-1] == "The distance must be greater than zero."
    h.run("2")
    assert h.flat(h.a) == pytest.approx((0.0, 0.0, 20.0, 0.0))
    h.select(h.a)
    h.run("SCALE", "0,0")
    assert h.prompt().drag == Drag("scale", (h.a,))
    h.runner.answer_point((0.0, 0.5))
    assert h.flat(h.a) == pytest.approx((0.0, 0.0, 10.0, 0.0))


def test_mirror_keeps_the_source_unless_told_to_erase_it() -> None:
    h = Harness()
    h.select(h.a)
    h.run("MI", "0,2", "10,2")
    assert h.prompt().text == "Erase source objects? [Yes(Y)/No(N)] <N>:"
    h.run("")
    ids = h.ids()
    assert len(ids) == 3 and h.line(h.a) == ((0.0, 0.0), (10.0, 0.0))
    assert h.flat(ids[-1]) == pytest.approx((0.0, 4.0, 10.0, 4.0))
    assert h.document.undo_history[-1] == "MIRROR"
    h.run("UNDO")
    assert h.ids() == [h.a, h.b], "the copy and its mirroring are one step"
    h.select(h.b)
    h.run("MIRROR", "0,0", "10,0", "Y")
    assert h.flat(h.b) == pytest.approx((0.0, -5.0, 10.0, -5.0))
    h.select(h.b)
    h.run("MIRROR", "1,1", "1,1")
    assert h.messages()[-1] == "The two points must differ."


def test_spanish_names_of_the_editing_commands() -> None:
    from arqstudiocad.i18n import set_language

    h = Harness()
    set_language("es")
    found = {
        typed: h.runner.registry.find(typed)
        for typed in (
            "BORRA",
            "DESPLAZA",
            "COPIA",
            "GIRA",
            "GI",
            "ESCALA",
            "ES",
            "SIMETRÍA",
            "SI",
            "DESCOMPONER",
        )
    }
    assert {typed: c.name if c else None for typed, c in found.items()} == {
        "BORRA": "ERASE",
        "DESPLAZA": "MOVE",
        "COPIA": "COPY",
        "GIRA": "ROTATE",
        "GI": "ROTATE",
        "ESCALA": "SCALE",
        "ES": "SCALE",
        "SIMETRÍA": "MIRROR",
        "SI": "MIRROR",
        "DESCOMPONER": "EXPLODE",
    }
    h.run("BORRA")
    assert h.prompt().text == "Designe objetos [Todo(ALL)/Último(L)/Previo(P)]:"


# In the window


@pytest.fixture
def window(qtbot: QtBot) -> MainWindow:
    ui = QuietUi()
    w = MainWindow(ui=ui)
    ui.view_for = w.viewport_for
    qtbot.addWidget(w)
    w.resize(900, 600)
    w.show()
    qtbot.waitExposed(w)
    return w


@pytest.mark.parametrize(
    ("command", "answers", "cursor", "moved"),
    [
        # The line (0,0)-(100,0): where its end (100,0) is drawn while dragging.
        ("MOVE", ("0,0",), (20.0, 30.0), (120.0, 30.0)),
        ("ROTATE", ("0,0",), (0.0, 30.0), (0.0, 100.0)),
        ("SCALE", ("0,0",), (0.5, 0.0), (50.0, 0.0)),
        ("MIRROR", ("0,0",), (30.0, 30.0), (0.0, 100.0)),
    ],
)
def test_the_selection_follows_the_cursor_transformed(
    window: MainWindow,
    tmp_path: Path,
    command: str,
    answers: tuple[str, ...],
    cursor: tuple[float, float],
    moved: tuple[float, float],
) -> None:
    viewport = plan(window, tmp_path)
    document = window.session.active
    assert document is not None
    line_id = document.get_entities()[0].id
    window.session.drafting.set_osnap(False)
    window.session.select(document, [line_id])
    window.runner.execute(command)
    for answer in answers:
        window.runner.execute(answer)
    assert viewport.preview is None, "nothing to show until the cursor moves"
    mouse(viewport, QEvent.Type.MouseMove, cursor)
    preview = viewport.preview
    assert preview is not None
    shown = preview.map(QPointF(*viewport.view.to_screen(100.0, 0.0)))
    expected = viewport.view.to_screen(*moved)
    assert (shown.x(), shown.y()) == pytest.approx(expected, abs=1.0)
    window.runner.cancel()
    assert viewport.preview is None


def test_keys_typed_in_the_viewport_go_to_the_command_line(
    window: MainWindow, tmp_path: Path
) -> None:
    viewport = plan(window, tmp_path)
    window.command_window.hide()
    for key, text in ((Qt.Key.Key_L, "L"), (Qt.Key.Key_Return, "\r")):
        event = QKeyEvent(QEvent.Type.KeyPress, key, Qt.KeyboardModifier.NoModifier, text)
        QApplication.sendEvent(viewport, event)
    assert window.runner.active == "LINE"
    assert window.command_window.isVisible(), "shown again to take the keys"


def test_the_erase_action_erases_the_selection_with_the_delete_key(
    window: MainWindow, tmp_path: Path
) -> None:
    plan(window, tmp_path)
    document = window.session.active
    assert document is not None
    first = document.get_entities()[0].id
    window.session.select(document, [first])
    action = window.command_actions["ERASE"]
    assert (
        action.shortcut().matches(QKeySequence(Qt.Key.Key_Delete))
        == QKeySequence.SequenceMatch.ExactMatch
    )
    action.trigger()
    assert [e.id for e in document.get_entities()] != [] and first not in [
        e.id for e in document.get_entities()
    ]


def escape(widget: object) -> None:
    event = QKeyEvent(QEvent.Type.KeyPress, Qt.Key.Key_Escape, Qt.KeyboardModifier.NoModifier)
    QApplication.sendEvent(widget, event)  # type: ignore[arg-type]


def test_esc_in_the_drawing_ends_a_drawing_command(window: MainWindow, tmp_path: Path) -> None:
    viewport = plan(window, tmp_path)
    document = window.session.active
    assert document is not None
    before = len(document.get_entities())
    window.session.drafting.set_osnap(False)
    window.runner.execute("LINE")
    for world in ((10.0, 60.0), (30.0, 60.0)):  # clicks give the viewport the focus
        mouse(viewport, QEvent.Type.MouseMove, world)
        mouse(viewport, QEvent.Type.MouseButtonPress, world)
    escape(viewport)
    assert window.runner.active is None, "Esc in the drawing cancels, as on the command line"
    assert viewport.target is None
    assert len(document.get_entities()) == before + 1, "the segment drawn stays"
    assert window.command_window.history.toPlainText().splitlines()[-1] == "*Cancel*"


def test_esc_in_the_drawing_ends_an_editing_command(window: MainWindow, tmp_path: Path) -> None:
    viewport = plan(window, tmp_path)
    document = window.session.active
    assert document is not None
    line_id = document.get_entities()[0].id
    window.session.drafting.set_osnap(False)
    window.session.select(document, [line_id])
    window.runner.execute("MOVE")
    window.runner.execute("0,0")
    mouse(viewport, QEvent.Type.MouseMove, (20.0, 30.0))
    assert viewport.preview is not None
    escape(viewport)
    assert window.runner.active is None and viewport.preview is None
    assert window.session.selection(document) == ()
    geometry = document.get_entity(line_id).geometry
    assert geometry == CadLine((0.0, 0.0), (100.0, 0.0)), "nothing moved"


def test_explode_is_in_the_modify_menu_ribbon_and_toolbar(window: MainWindow) -> None:
    from PySide6.QtWidgets import QMenu, QToolBar

    from arqstudiocad.ui.ribbon import RibbonGroupWidget

    action = window.command_actions["EXPLODE"]
    assert action.text() == "E&xplode"
    assert any(action in menu.actions() for menu in window.menuBar().findChildren(QMenu))
    groups = {g.group.name: g for g in window.findChildren(RibbonGroupWidget)}
    assert action in [b.defaultAction() for b in groups["modify"].buttons]
    toolbars = [bar for bar in window.findChildren(QToolBar) if action in bar.actions()]
    assert toolbars, "the Classic Modify toolbar"
