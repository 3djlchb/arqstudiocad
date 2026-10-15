"""LINE, CIRCLE and ARC (ADR 0016 D8) and the command names a language adds
(ADR 0006, ADR 0016 D9)."""

import json
import math
from pathlib import Path

import pytest
from PySide6.QtCore import QEvent
from PySide6.QtWidgets import QApplication
from pytestqt.qtbot import QtBot
from test_point_input import mouse, plan
from ui_fakes import QuietUi

from arqstudiocad.commands import CommandContext, CommandRunner
from arqstudiocad.commands.builtin import builtin_registry
from arqstudiocad.core import CadArc, CadCircle, CadLine
from arqstudiocad.events import CommandMessage, CommandPrompt, Event, EventBus
from arqstudiocad.i18n import LanguagePackError, discover, load_pack, set_language
from arqstudiocad.paths import locales_dir
from arqstudiocad.session import Session
from arqstudiocad.ui.main_window import MainWindow


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

    def messages(self) -> list[str]:
        return [e.text for e in self.events if isinstance(e, CommandMessage)]

    def prompt(self) -> CommandPrompt:
        return [e for e in self.events if isinstance(e, CommandPrompt)][-1]

    def curves(self) -> list[CadLine | CadCircle | CadArc]:
        found = [e.geometry for e in self.document.get_entities()]
        return [g for g in found if g is not None]


def ends(line: CadLine | CadCircle | CadArc) -> tuple[tuple[float, float], tuple[float, float]]:
    assert isinstance(line, CadLine)
    return line.start, line.end


# LINE


def test_line_draws_segments_until_enter_as_one_undo_step() -> None:
    h = Harness()
    h.run("LINE", "0,0", "10,0")
    assert h.prompt().text == "Specify next point [Undo(U)]:"
    assert h.prompt().base == (10.0, 0.0), "the rubber band starts at the last point"
    h.run("@0,5", "")
    assert h.runner.active is None
    assert [ends(c) for c in h.curves()] == [((0, 0), (10, 0)), ((10, 0), (10, 5))]
    assert h.document.undo_history == ["LINE"], "the whole run is one undo step"


def test_line_undo_takes_back_the_last_segment_and_close_joins_the_first_point() -> None:
    h = Harness()
    h.run("L", "0,0", "10,0", "10,10", "U")
    assert [ends(c) for c in h.curves()] == [((0, 0), (10, 0))]
    assert h.prompt().base == (10.0, 0.0), "the next segment starts where the undone one did"
    assert h.prompt().text == "Specify next point [Undo(U)]:", "Close needs two segments"
    h.run("U", "U")
    assert h.messages()[-1] == "All segments already undone."
    h.run("10,0", "10,10")
    assert h.prompt().text == "Specify next point [Undo(U)/Close(C)]:"
    h.run("C")
    assert h.runner.active is None
    assert [ends(c) for c in h.curves()] == [
        ((0, 0), (10, 0)),
        ((10, 0), (10, 10)),
        ((10, 10), (0, 0)),
    ]
    assert h.document.undo_history == ["LINE"]


def test_line_refuses_a_point_on_the_last_one_and_goes_on() -> None:
    h = Harness()
    h.run("LINE", "1,1", "1,1")
    assert h.messages()[-1] == "The two points must differ."
    assert h.runner.active == "LINE"
    h.run("2,1", "")
    assert [ends(c) for c in h.curves()] == [((1, 1), (2, 1))]


def test_esc_keeps_the_segments_drawn_and_an_empty_line_leaves_no_step() -> None:
    h = Harness()
    h.run("LINE", "0,0", "5,0", "5,5")
    h.runner.cancel()
    assert len(h.curves()) == 2
    assert h.document.undo_history == ["LINE"] and not h.document.in_group
    h.run("LINE")
    h.runner.cancel()
    assert h.document.undo_history == ["LINE"], "nothing drawn, no step"
    h.run("UNDO")
    assert h.curves() == []


def test_a_picked_point_answers_line() -> None:
    h = Harness()
    h.run("LINE")
    h.runner.answer_point((2.0, 3.0))
    h.runner.answer_point((4.0, 3.0))
    h.run("")
    assert [ends(c) for c in h.curves()] == [((2, 3), (4, 3))]


# CIRCLE


def test_circle_by_center_and_radius_remembers_the_radius() -> None:
    h = Harness()
    h.run("CIRCLE")
    assert h.prompt().text == "Specify center point for circle [3P/2P]:"
    h.run("10,10")
    assert h.prompt().text == "Specify radius of circle [Diameter(D)]:"
    h.run("2.5")
    assert h.curves() == [CadCircle((10.0, 10.0), 2.5)]
    h.run("C", "0,0")
    assert h.prompt().text == "Specify radius of circle [Diameter(D)] <2.5>:"
    h.run("")
    assert h.curves()[-1] == CadCircle((0.0, 0.0), 2.5)
    h.run("C", "0,0", "D")
    assert h.prompt().text == "Specify diameter of circle <5>:"
    h.run("8")
    assert h.curves()[-1] == CadCircle((0.0, 0.0), 4.0)


def test_a_radius_is_also_the_distance_to_a_point() -> None:
    h = Harness()
    h.run("CIRCLE", "1,1")
    assert h.prompt().point and h.prompt().base == (1.0, 1.0), "picked in the viewport"
    h.runner.answer_point((4.0, 5.0))
    assert h.curves() == [CadCircle((1.0, 1.0), 5.0)]
    h.run("CIRCLE", "1,1", "@0,2")
    assert h.curves()[-1] == CadCircle((1.0, 1.0), 2.0)


def test_a_radius_must_be_positive() -> None:
    h = Harness()
    h.run("CIRCLE", "0,0")
    for typed in ("0", "-3", "nan", "1,1"):
        h.run("0,0" if typed == "1,1" else typed)
        assert h.messages()[-1] == "The distance must be greater than zero."
    h.run("abc")
    assert h.messages()[-1] == "abc is not a number."
    assert h.runner.active == "CIRCLE" and h.curves() == []


def test_circle_by_two_and_three_points() -> None:
    h = Harness()
    h.run("CIRCLE", "2P", "1,1", "5,1")
    assert h.curves() == [CadCircle((3.0, 1.0), 2.0)]
    h.run("CIRCLE", "3p", "15,5", "10,10", "5,5")
    circle = h.curves()[-1]
    assert isinstance(circle, CadCircle)
    assert circle.center == pytest.approx((10.0, 5.0)) and circle.radius == pytest.approx(5.0)
    h.run("CIRCLE", "3P", "0,0", "1,1", "2,2")
    assert h.messages()[-1] == "Three points on one line define no circle."
    assert h.runner.active is None and len(h.curves()) == 2


# ARC


def test_arc_through_three_points() -> None:
    h = Harness()
    h.run("ARC")
    assert h.prompt().text == "Specify start point of arc [Center(C)]:"
    h.run("5,5")
    assert h.prompt().text == "Specify second point of arc [Center(C)/End(E)]:"
    h.run("10,10", "15,5")
    arc = h.curves()[0]
    assert isinstance(arc, CadArc)
    assert arc.center == pytest.approx((10.0, 5.0)) and arc.radius == pytest.approx(5.0)
    assert arc.start_point == pytest.approx((15.0, 5.0)), "stored counterclockwise"
    assert arc.end_point == pytest.approx((5.0, 5.0))
    h.run("A", "0,0", "1,1", "2,2")
    assert h.messages()[-1] == "Three points on one line define no arc."
    assert len(h.curves()) == 1


@pytest.mark.parametrize(
    "lines",
    [
        ("C", "0,0", "10,0", "0,3"),  # center, start, end
        ("10,0", "C", "0,0", "0,3"),  # start, center, end
        ("10,0", "E", "0,3", "0,0"),  # start, end, center
    ],
)
def test_arc_by_start_center_and_end(lines: tuple[str, ...]) -> None:
    h = Harness()
    h.run("ARC", *lines)
    arc = h.curves()[0]
    assert isinstance(arc, CadArc)
    assert arc.center == pytest.approx((0.0, 0.0)) and arc.radius == pytest.approx(10.0)
    assert (arc.start_angle, arc.end_angle) == pytest.approx((0.0, math.pi / 2))


def test_arc_refuses_an_end_in_the_start_direction() -> None:
    h = Harness()
    h.run("ARC", "C", "0,0", "10,0", "5,0")
    assert h.messages()[-1] == (
        "The start, the center and the end must be three different directions."
    )
    assert h.curves() == []


# Command names in Spanish (ADR 0016 D9)

#: AutoCAD's standard English aliases (acad.pgp): kept for the English
#: commands that will use them, so a language never takes one.
AUTOCAD_ALIASES = {
    "A",
    "AA",
    "AL",
    "AR",
    "ATT",
    "B",
    "BH",
    "BO",
    "BR",
    "C",
    "CH",
    "CO",
    "CP",
    "D",
    "DI",
    "DIV",
    "DO",
    "DT",
    "E",
    "ED",
    "EL",
    "EX",
    "F",
    "FI",
    "G",
    "H",
    "HE",
    "I",
    "IN",
    "IO",
    "J",
    "L",
    "LA",
    "LE",
    "LEN",
    "LI",
    "LO",
    "LT",
    "LTS",
    "LW",
    "M",
    "MA",
    "ME",
    "MI",
    "ML",
    "MO",
    "MS",
    "MT",
    "MV",
    "O",
    "OP",
    "OS",
    "P",
    "PE",
    "PL",
    "PO",
    "POL",
    "PR",
    "PRE",
    "PU",
    "R",
    "RE",
    "REA",
    "REC",
    "REG",
    "REN",
    "RO",
    "S",
    "SC",
    "SCR",
    "SE",
    "SET",
    "SN",
    "SO",
    "SP",
    "SPL",
    "ST",
    "T",
    "TA",
    "TH",
    "TI",
    "TO",
    "TOR",
    "TR",
    "U",
    "UN",
    "V",
    "W",
    "X",
    "XL",
    "XR",
    "Z",
}


def test_spanish_names_cover_every_command_and_never_shadow_english() -> None:
    names = json.loads((locales_dir() / "es" / "commands.json").read_text(encoding="utf-8"))
    registry = builtin_registry()
    english = {key for key in registry.names()}
    commands = {registry.find(key).name for key in english}  # type: ignore[union-attr]
    assert set(names) == commands, "every command gets its Spanish name in its own step"
    spanish = [name for added in names.values() for name in added]
    assert "LINEA" in spanish and len(spanish) == len(set(spanish))
    assert not set(spanish) & english, "a pack adds names, never shadows an English one"
    assert not set(spanish) & AUTOCAD_ALIASES, "nor takes an alias AutoCAD gives in English"


def test_spanish_names_work_with_or_without_accents_and_english_still_works() -> None:
    h = Harness()
    assert h.runner.registry.find("LINEA") is None, "only in Spanish"
    set_language("es")
    find = h.runner.registry.find
    for typed in ("LINEA", "línea", "LÍNEA", " Linea "):
        assert find(typed) is not None and find(typed).name == "LINE"  # type: ignore[union-attr]
    assert find("CÍRCULO").name == "CIRCLE"  # type: ignore[union-attr]
    assert find("ca").name == "LAYER"  # type: ignore[union-attr]
    assert find("_LINEA") is None, "_ forces the English name"
    assert find("_LINE").name == find("LINE").name == "LINE"  # type: ignore[union-attr]
    assert {"LINEA", "CIRCULO", "ARCO", "GUARDAR"} <= set(h.runner.registry.names())
    h.run("CÍRCULO", "0,0", "1")
    assert h.curves() == [CadCircle((0.0, 0.0), 1.0)]
    assert h.prompt().text == "Precise radio de círculo [Diámetro(D)]:"


def test_a_pack_must_write_command_names_as_autocad_does(tmp_path: Path) -> None:
    folder = tmp_path / "xx"
    folder.mkdir()
    (folder / "meta.json").write_text('{"code": "xx", "name": "X"}', encoding="utf-8")
    good = load_pack(folder)
    assert good.commands == {}
    for bad in ('{"LINE": ["línea"]}', '{"LINE": ["LÍNEA"]}', '{"LINE": "LINEA"}'):
        (folder / "commands.json").write_text(bad, encoding="utf-8")
        with pytest.raises(LanguagePackError, match="upper-case ASCII"):
            load_pack(folder)
    assert discover(strict=True)["es"].commands["LINE"] == ("LINEA",)


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


def test_the_draw_menu_starts_line_and_clicks_draw_it_at_the_snaps(
    window: MainWindow, tmp_path: Path
) -> None:
    viewport = plan(window, tmp_path)  # a line (0,0)-(100,0), a circle at (50,40)
    before = len(window.session.active.get_entities())  # type: ignore[union-attr]
    window.command_actions["LINE"].trigger()
    assert window.runner.active == "LINE"
    for world in ((99.0, 1.0), (60.5, 40.0)):  # END of the line, CEN of the circle
        mouse(viewport, QEvent.Type.MouseMove, world)
        mouse(viewport, QEvent.Type.MouseButtonPress, world)
    window.runner.execute("")
    QApplication.processEvents()
    entities = window.session.active.get_entities()  # type: ignore[union-attr]
    assert len(entities) == before + 1
    assert entities[-1].geometry == CadLine((100.0, 0.0), (50.0, 40.0))


def test_the_command_line_completes_the_languages_names(window: MainWindow) -> None:
    completer = window.command_window.input.completer()
    assert completer is not None
    assert "LINEA" not in completer.model().stringList()  # type: ignore[attr-defined]
    window.change_language("es")
    assert "LINEA" in completer.model().stringList()  # type: ignore[attr-defined]
