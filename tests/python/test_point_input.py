"""Point prompts (ADR 0016 D5, D6): typed in AutoCAD's syntax or picked in
the viewport with running snaps and ORTHO; the status bar's switches."""

from pathlib import Path

import pytest
from PySide6.QtCore import QEvent, QPointF, Qt
from PySide6.QtGui import QMouseEvent
from PySide6.QtWidgets import QApplication
from pytestqt.qtbot import QtBot
from ui_fakes import QuietUi

from arqstudiocad.commands import (
    Command,
    CommandContext,
    CommandRunner,
    CommandScript,
)
from arqstudiocad.commands.builtin import builtin_registry
from arqstudiocad.drafting import Drafting
from arqstudiocad.events import (
    CommandMessage,
    CommandPrompt,
    DraftingChanged,
    Event,
    EventBus,
)
from arqstudiocad.i18n import set_language
from arqstudiocad.session import Session
from arqstudiocad.ui.main_window import MainWindow
from arqstudiocad.ui.viewport import Viewport

# The command system, no Qt


def from_origin(ctx: CommandContext) -> CommandScript:
    """A test command: one point from (0, 0), as the second point of a line."""
    point = yield ctx.point("Next point", base=(0.0, 0.0))
    ctx.message(f"got {point[0]:g},{point[1]:g}")


class Harness:
    def __init__(self) -> None:
        self.bus = EventBus()
        self.events: list[Event] = []
        self.bus.subscribe(Event, self.events.append)
        self.session = Session(self.bus)
        registry = builtin_registry()
        registry.register(Command("FROMORIGIN", from_origin))
        self.runner = CommandRunner(registry, CommandContext(self.session))
        self.session.new_document()

    def messages(self) -> list[str]:
        return [e.text for e in self.events if isinstance(e, CommandMessage)]

    def prompts(self) -> list[CommandPrompt]:
        return [e for e in self.events if isinstance(e, CommandPrompt)]


def test_id_shows_a_typed_point() -> None:
    h = Harness()
    h.runner.execute("ID")
    prompt = h.prompts()[-1]
    assert (prompt.text, prompt.point, prompt.base) == ("Specify point:", True, None)
    h.runner.execute("3,4")
    assert h.messages()[-1] == "X = 3.0000     Y = 4.0000"
    assert h.runner.active is None


def test_dist_measures_between_two_points_with_a_rubber_band() -> None:
    h = Harness()
    h.runner.execute("DI")
    assert h.prompts()[-1].text == "Specify first point:"
    h.runner.execute("1,2")
    prompt = h.prompts()[-1]
    assert (prompt.text, prompt.base) == ("Specify second point:", (1.0, 2.0)), "a rubber band"
    h.runner.execute("@-8,-6")
    assert h.messages()[-2:] == [
        "Distance = 10.0000,  Angle in XY plane = 216.8699°",
        "Delta X = -8.0000,  Delta Y = -6.0000",
    ]
    assert h.runner.active is None
    h.runner.execute("DIST")  # AutoCAD's name
    h.runner.execute("0,0")
    h.runner.execute("0,0")
    assert h.messages()[-2] == "Distance = 0.0000,  Angle in XY plane = 0.0000°"


def test_the_english_names_and_their_aliases() -> None:
    h = Harness()
    for typed, name in (
        ("IDPOINT", "IDPOINT"),
        ("ID", "IDPOINT"),
        ("DISTANCE", "DISTANCE"),
        ("DI", "DISTANCE"),
        ("DIST", "DISTANCE"),
    ):
        h.runner.execute(typed)
        assert h.runner.active == name, typed
        h.runner.cancel()


def test_id_and_dist_in_spanish() -> None:
    set_language("es")
    h = Harness()
    h.runner.execute("IDPUNTO")
    h.runner.execute("3,4")
    assert h.messages()[-1] == "X = 3.0000     Y = 4.0000"
    h.runner.execute("DISTANCIA")
    h.runner.execute("0,0")
    h.runner.execute("3,4")
    assert h.messages()[-2] == "Distancia = 5.0000,  Ángulo en plano XY = 53.1301°"


def test_a_wrong_point_says_why_and_asks_again() -> None:
    h = Harness()
    h.runner.execute("ID")
    h.runner.execute("abc")
    assert h.messages()[-1] == "abc is not a number."
    h.runner.execute("@1,1")
    assert h.messages()[-1] == "There is no last point."
    assert h.runner.active == "IDPOINT" and h.prompts()[-1].text == "Specify point:"
    h.runner.execute("5,6")
    assert h.messages()[-1] == "X = 5.0000     Y = 6.0000"


def test_relative_points_and_direct_distance_use_the_base() -> None:
    h = Harness()
    h.runner.execute("FROMORIGIN")
    assert h.prompts()[-1].base == (0.0, 0.0)
    h.runner.execute("@2<90")
    assert h.messages()[-1] == "got 1.22465e-16,2"
    h.session.drafting.cursor = (3.0, 4.0)
    h.runner.execute("FROMORIGIN")
    h.runner.execute("10")
    assert h.messages()[-1] == "got 6,8"


def test_a_typed_snap_applies_to_the_next_point_only() -> None:
    h = Harness()
    drafting = h.session.drafting
    drafting.set_osnap(False)
    h.runner.execute("ID")
    h.runner.execute("end")
    assert drafting.override == "END" and drafting.active_snaps() == ("END",)
    assert h.prompts()[-1].text == "Specify point:", "asked again"
    h.runner.answer_point((1.0, 2.0))
    assert h.messages()[-1] == "X = 1.0000     Y = 2.0000"
    assert drafting.override is None and drafting.active_snaps() == ()
    h.runner.execute("ID")
    h.runner.execute("MID")
    h.runner.cancel()
    assert drafting.override is None, "cancelling forgets it too"


def test_a_picked_point_answers_only_a_point_prompt() -> None:
    h = Harness()
    h.runner.answer_point((1.0, 1.0))  # no command: nothing happens
    h.runner.execute("OPEN")  # without an interface it asks for a file name
    h.runner.answer_point((1.0, 1.0))
    assert h.runner.active == "OPEN", "a file name is not a point"


def test_drafting_switches_publish_and_decide_the_active_snaps() -> None:
    bus = EventBus()
    seen: list[DraftingChanged] = []
    bus.subscribe(DraftingChanged, seen.append)
    d = Drafting(bus)
    assert (d.osnap, d.snaps, d.ortho) == (True, ("END", "MID", "CEN", "INT"), False)
    d.set_ortho(True)
    d.set_snaps({"QUA", "END"})
    d.set_osnap(False)
    d.set_osnap(False)
    assert seen == [
        DraftingChanged(True, ("END", "MID", "CEN", "INT"), True),
        DraftingChanged(True, ("END", "QUA"), True),
        DraftingChanged(False, ("END", "QUA"), True),
    ]
    assert d.active_snaps() == ()
    d.set_osnap(True)
    assert d.active_snaps() == ("END", "QUA")
    with pytest.raises(ValueError, match="unknown object snaps"):
        d.set_snaps({"ENDPOINT"})


# The viewport and the status bar


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


def plan(window: MainWindow, tmp_path: Path) -> Viewport:
    """A line (0,0)-(100,0) and a circle at (50,40), radius 10."""
    codes = ["0", "SECTION", "2", "ENTITIES"]
    codes += [
        "0",
        "LINE",
        "8",
        "0",
        "10",
        "0",
        "20",
        "0",
        "30",
        "0",
        "11",
        "100",
        "21",
        "0",
        "31",
        "0",
    ]
    codes += ["0", "CIRCLE", "8", "0", "10", "50", "20", "40", "30", "0", "40", "10"]
    codes += ["0", "ENDSEC", "0", "EOF", ""]
    path = tmp_path / "plano.dxf"
    path.write_text("\n".join(codes), encoding="ascii")
    document = window.session.open_document(path)
    QApplication.processEvents()
    return window.viewports[document.id]


def mouse(viewport: Viewport, kind: QEvent.Type, world: tuple[float, float]) -> None:
    point = QPointF(*viewport.view.to_screen(*world))
    button = Qt.MouseButton.LeftButton if kind != QEvent.Type.MouseMove else Qt.MouseButton.NoButton
    held = Qt.MouseButton.NoButton
    event = QMouseEvent(
        kind, point, viewport.mapToGlobal(point), button, held, Qt.KeyboardModifier.NoModifier
    )
    QApplication.sendEvent(viewport, event)


def history(window: MainWindow) -> list[str]:
    return window.command_window.history.toPlainText().splitlines()


def test_a_click_answers_a_point_prompt_at_the_snap(window: MainWindow, tmp_path: Path) -> None:
    viewport = plan(window, tmp_path)
    window.runner.execute("ID")
    mouse(viewport, QEvent.Type.MouseMove, (99.0, 1.0))
    assert viewport.snap is not None and viewport.snap.kind == "END"
    assert viewport.target == (100.0, 0.0)
    mouse(viewport, QEvent.Type.MouseButtonPress, (99.0, 1.0))
    assert history(window)[-1] == "X = 100.0000     Y = 0.0000"
    assert viewport.target is None, "no point asked for any more"
    assert window.session.selection(window.session.active) == (), "not a selection click"


def test_the_snap_marker_is_painted_with_its_name(window: MainWindow, tmp_path: Path) -> None:
    viewport = plan(window, tmp_path)
    window.runner.execute("ID")
    # On the circle: Quadrant is not a default snap, so Center answers.
    mouse(viewport, QEvent.Type.MouseMove, (60.5, 40.0))
    assert viewport.snap is not None and viewport.snap.kind == "CEN"
    assert viewport.target == (50.0, 40.0)
    image = viewport.grab().toImage()
    sx, sy = viewport.view.to_screen(*viewport.target)
    yellow = 0
    for dx in range(-8, 9):
        for dy in range(-8, 9):
            c = image.pixelColor(round(sx) + dx, round(sy) + dy)
            yellow += c.red() > 200 and c.green() > 150 and c.blue() < 80
    assert yellow > 4, "the marker is drawn in AutoSnap yellow"


def test_without_running_snaps_the_cursor_is_the_point_and_ortho_keeps_it_straight(
    window: MainWindow, tmp_path: Path
) -> None:
    viewport = plan(window, tmp_path)
    window.drafting_bar.osnap_action.trigger()  # F3: off
    assert window.session.drafting.osnap is False
    window.runner.registry.register(Command("FROMORIGIN", from_origin))
    window.runner.execute("FROMORIGIN")
    mouse(viewport, QEvent.Type.MouseMove, (40.0, 7.0))
    assert viewport.snap is None
    assert viewport.target == pytest.approx((40.0, 7.0), abs=0.5)
    window.drafting_bar.ortho_action.trigger()  # F8: on
    mouse(viewport, QEvent.Type.MouseMove, (40.0, 7.0))
    assert viewport.target is not None and viewport.target[1] == 0.0, "horizontal from (0,0)"
    mouse(viewport, QEvent.Type.MouseButtonPress, (40.0, 7.0))
    assert history(window)[-1].startswith("got ") and history(window)[-1].endswith(",0")


def test_the_status_bar_shows_and_changes_the_drafting_state(
    window: MainWindow, tmp_path: Path
) -> None:
    bar = window.drafting_bar
    drafting = window.session.drafting
    assert bar.osnap_action.isChecked() and not bar.ortho_action.isChecked()
    assert bar.osnap_action.shortcut().toString() == "F3"
    assert bar.ortho_action.shortcut().toString() == "F8"
    drafting.set_ortho(True)
    assert bar.ortho_action.isChecked(), "the session's state is shown"
    bar.snap_actions["QUA"].trigger()
    assert drafting.snaps == ("END", "MID", "CEN", "QUA", "INT")
    assert [k for k, a in bar.snap_actions.items() if a.isChecked()] == list(drafting.snaps)
    assert bar.osnap_action.text() == "OSNAP" and bar.snap_actions["END"].text() == "Endpoint"


def test_esc_ends_the_point_prompt(window: MainWindow, tmp_path: Path) -> None:
    viewport = plan(window, tmp_path)
    window.runner.execute("ID")
    mouse(viewport, QEvent.Type.MouseMove, (99.0, 1.0))
    assert viewport.target is not None
    window.runner.cancel()
    assert viewport.target is None and viewport.snap is None


def test_a_distance_typed_alone_goes_where_the_cursor_points(
    window: MainWindow, tmp_path: Path
) -> None:
    viewport = plan(window, tmp_path)
    window.drafting_bar.osnap_action.trigger()  # off: no snap moves the cursor
    window.runner.registry.register(Command("FROMORIGIN", from_origin))
    window.runner.execute("FROMORIGIN")
    mouse(viewport, QEvent.Type.MouseMove, (0.0, 30.0))  # straight up from (0,0)
    window.runner.execute("10")
    x, y = (float(v) for v in history(window)[-1].removeprefix("got ").split(","))
    assert (x, y) == pytest.approx((0.0, 10.0), abs=1e-9)


def two_points(ctx: CommandContext) -> CommandScript:
    """A test command: two points, as a line asks for them."""
    first = yield ctx.point("First point")
    yield ctx.point("Next point", base=first)


def test_a_snap_override_ends_with_the_point_it_was_for() -> None:
    h = Harness()
    h.runner.registry.register(Command("TWOPOINTS", two_points))
    h.runner.execute("TWOPOINTS")
    h.runner.execute("cen")
    h.runner.answer_point((1.0, 2.0))
    assert h.prompts()[-1].text == "Next point:" and h.prompts()[-1].base == (1.0, 2.0)
    assert h.session.drafting.override is None, "the second point snaps as usual"
