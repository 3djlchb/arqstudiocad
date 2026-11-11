"""RECTANG (milestone 02.14, ADR 0035 D4): AutoCAD's rectangle by two
corners, Area, Dimensions and Rotation, its corners square, filleted or
chamfered, as one closed polyline; the values it remembers, its prompt
for a number, its preview, its name in Spanish and its place in the
interface. Every expected shape is worked out by hand."""

import math
from pathlib import Path

import pytest
from pytestqt.qtbot import QtBot
from ui_fakes import QuietUi

from arqstudiocad.commands import CommandContext, CommandRunner
from arqstudiocad.commands.builtin import builtin_registry
from arqstudiocad.core import CadDocument, CadRectangle, InvalidGeometryError
from arqstudiocad.events import CommandMessage, CommandPrompt, Drag, Event, EventBus
from arqstudiocad.i18n import set_language
from arqstudiocad.session import Session
from arqstudiocad.ui.main_window import MainWindow

OTHER = "Specify other corner point [Area(A)/Dimensions(D)/Rotation(R)]:"
FIRST = "Specify first corner point [Chamfer(C)/Fillet(F)]:"
Q = math.tan(math.pi / 8)  # a quarter circle's bulge


class Harness:
    def __init__(self) -> None:
        self.bus = EventBus()
        self.events: list[Event] = []
        self.bus.subscribe(Event, self.events.append)
        self.session = Session(self.bus)
        self.runner = CommandRunner(builtin_registry(), CommandContext(self.session))
        self.document: CadDocument = self.session.new_document()

    def run(self, *lines: str) -> None:
        for line in lines:
            self.runner.execute(line)

    def prompt(self) -> CommandPrompt:
        return [e for e in self.events if isinstance(e, CommandPrompt)][-1]

    def messages(self) -> list[str]:
        return [e.text for e in self.events if isinstance(e, CommandMessage)]

    def polylines(self) -> list[int]:
        return [e.id for e in self.document.get_entities() if e.kind == "LWPOLYLINE"]

    def vertices(self, index: int = -1) -> list[tuple[float, float]]:
        polyline = self.polylines()[index]
        grips = [g for g in self.document.grips([polyline]) if g.vertex_index is not None]
        ordered = sorted(grips, key=lambda g: g.vertex_index or 0)
        return [(round(g.point[0], 9) + 0.0, round(g.point[1], 9) + 0.0) for g in ordered]


def test_two_corners_draw_one_closed_polyline_as_one_undo_step() -> None:
    h = Harness()
    h.run("RECTANG")
    assert h.prompt().text == FIRST
    h.run("0,0")
    assert h.prompt().text == OTHER
    assert h.prompt().drag == Drag("rectangle", band=False, rotation=0.0)
    h.run("4,3")
    assert h.vertices() == [(0.0, 0.0), (4.0, 0.0), (4.0, 3.0), (0.0, 3.0)]
    assert h.document.undo_history == ["RECTANG"]
    [only] = h.polylines()
    assert ("Closed", "true") in h.document.get_entity(only).details
    assert h.runner.active is None, "the command ends"


def test_the_step_is_in_the_history_when_the_change_is_reported() -> None:
    from arqstudiocad.events import DocumentChanged

    h = Harness()
    seen: list[list[str]] = []
    h.bus.subscribe(DocumentChanged, lambda e: seen.append(list(h.document.undo_history)))
    h.run("RECTANG", "0,0", "4,3")
    assert seen == [["RECTANG"]], "the History panel refreshes on this event"


def test_the_corners_run_counterclockwise_from_the_first_in_any_quarter() -> None:
    h = Harness()
    h.run("REC", "1,1", "-3,4")
    assert h.vertices() == [(1.0, 1.0), (1.0, 4.0), (-3.0, 4.0), (-3.0, 1.0)]
    h.run("RECTANGLE", "1,1", "@4,-2")
    assert h.vertices() == [(1.0, 1.0), (1.0, -1.0), (5.0, -1.0), (5.0, 1.0)]


def test_a_corner_on_a_side_line_is_asked_again() -> None:
    h = Harness()
    h.run("RECTANG", "0,0", "5,0")
    assert h.messages()[-1] == "The corners must differ in both directions."
    assert h.prompt().text == OTHER and not h.polylines()
    h.run("0,0")
    assert h.messages()[-1] == "The corners must differ in both directions."
    h.run("-2,-3")
    assert h.vertices() == [(0.0, 0.0), (-2.0, 0.0), (-2.0, -3.0), (0.0, -3.0)]


def test_dimensions_then_a_point_picks_the_quarter() -> None:
    h = Harness()
    h.run("RECTANG", "1,1", "D")
    assert h.prompt().text == "Specify length for rectangles <10>:", "AutoCAD's default"
    h.run("6")
    assert h.prompt().text == "Specify width for rectangles <10>:"
    h.run("2")
    assert h.prompt().text == OTHER
    assert h.prompt().drag == Drag("rectangle", band=False, rotation=0.0, size=(6.0, 2.0))
    h.run("-5,5")
    assert h.vertices() == [(1.0, 1.0), (1.0, 3.0), (-5.0, 3.0), (-5.0, 1.0)]
    # Remembered for the next rectangle; Enter takes them.
    h.run("RECTANG", "0,0", "D")
    assert h.prompt().text == "Specify length for rectangles <6>:"
    h.run("", "")
    h.run("1,-1")
    assert h.vertices() == [(0.0, 0.0), (0.0, -2.0), (6.0, -2.0), (6.0, 0.0)]


def test_area_with_the_length_or_the_width() -> None:
    h = Harness()
    h.run("RECTANG", "0,0", "A")
    assert h.prompt().text == "Enter area of rectangle in current units <100>:"
    h.run("12")
    assert h.prompt().text == "Calculate rectangle dimensions based on [Length(L)/Width(W)] <L>:"
    h.run("W")
    assert h.prompt().text == "Enter rectangle width <10>:"
    h.run("3")
    assert h.vertices() == [(0.0, 0.0), (4.0, 0.0), (4.0, 3.0), (0.0, 3.0)]
    # The area and its side are remembered, Length by default too.
    h.run("RECTANG", "10,0", "A", "")
    assert h.prompt().text == "Calculate rectangle dimensions based on [Length(L)/Width(W)] <W>:"
    h.run("L")
    assert h.prompt().text == "Enter rectangle length <10>:"
    h.run("6")
    assert h.vertices() == [(10.0, 0.0), (16.0, 0.0), (16.0, 2.0), (10.0, 2.0)]


@pytest.mark.parametrize("typed", ["abc", "0", "-5", "inf"])
def test_the_area_must_be_a_number_greater_than_zero(typed: str) -> None:
    h = Harness()
    h.run("RECTANG", "0,0", "A", typed)
    assert h.messages()[-1] == "Type a number greater than zero."
    assert h.prompt().text == "Enter area of rectangle in current units <100>:"
    assert not h.prompt().point, "an area is typed, not picked"
    # A point picked in the viewport does not answer it.
    h.runner.answer_point((3.0, 4.0))
    assert h.prompt().text == "Enter area of rectangle in current units <100>:"


def test_an_area_too_large_for_its_side_leaves_no_rectangle() -> None:
    h = Harness()
    h.run("RECTANG", "0,0", "A", "1e300", "L", "1e-300")
    assert h.messages()[-1] == "That area and side leave no rectangle."
    assert h.prompt().text == OTHER and not h.polylines()


def test_rotation_typed_turns_the_sides() -> None:
    h = Harness()
    h.run("RECTANG", "0,0", "R")
    assert h.prompt().text == "Specify rotation angle [Pick points(P)] <0>:"
    h.run("90")
    assert h.prompt().text == OTHER
    assert h.prompt().drag == Drag("rectangle", band=False, rotation=math.pi / 2)
    h.run("-2,4")
    assert h.vertices() == [(0.0, 0.0), (0.0, 4.0), (-2.0, 4.0), (-2.0, 0.0)]
    # Remembered: the next rectangle starts turned.
    h.run("RECTANG", "0,0", "R")
    assert h.prompt().text == "Specify rotation angle [Pick points(P)] <90>:"


def test_rotation_by_two_points_picked() -> None:
    h = Harness()
    h.run("RECTANG", "0,0", "R", "P")
    assert h.prompt().text == "Specify first point:"
    h.run("5,5")
    assert h.prompt().text == "Specify second point:"
    h.run("5,5")
    assert h.messages()[-1] == "The two points must differ."
    assert h.prompt().text == "Specify rotation angle [Pick points(P)] <0>:"
    h.run("P", "5,5", "6,6", "0,2")
    assert h.vertices() == [(0.0, 0.0), (1.0, 1.0), (0.0, 2.0), (-1.0, 1.0)]


def test_rotation_then_dimensions_and_area_follow_it() -> None:
    h = Harness()
    h.run("RECTANG", "0,0", "R", "90", "D", "4", "3", "-1,1")
    assert h.vertices() == [(0.0, 0.0), (0.0, 4.0), (-3.0, 4.0), (-3.0, 0.0)]
    h.run("RECTANG", "0,0", "A", "6", "L", "2")
    assert h.vertices() == [(0.0, 0.0), (0.0, 2.0), (-3.0, 2.0), (-3.0, 0.0)]


def test_esc_draws_nothing() -> None:
    h = Harness()
    h.run("RECTANG", "0,0", "D", "4")
    h.runner.cancel()
    assert not h.polylines() and h.document.undo_history == []


def test_the_rectangle_survives_saving(tmp_path: Path) -> None:
    h = Harness()
    h.run("RECTANG", "100000.5,200000.25", "R", "30", "D", "4", "3", "100010,200010")
    expected = h.vertices()
    for name in ("r.dwg", "r.dxf"):
        path = tmp_path / name
        h.session.save(h.document, path)
        again = h.session.open_document(path)
        [polyline] = [e.id for e in again.get_entities() if e.kind == "LWPOLYLINE"]
        grips = sorted(
            (g for g in again.grips([polyline]) if g.vertex_index is not None),
            key=lambda g: g.vertex_index or 0,
        )
        got = [g.point for g in grips]
        assert all(math.dist(a, b) < 1e-9 for a, b in zip(got, expected, strict=True)), name
        assert ("Closed", "true") in again.get_entity(polyline).details


def test_core_rectangle_values() -> None:
    r = CadRectangle((1.0, 2.0), (5.0, 4.0))
    assert (r.first, r.other, r.rotation) == ((1.0, 2.0), (5.0, 4.0), 0.0)
    assert (r.length, r.width, r.area) == (4.0, 2.0, 8.0)
    assert r.vertices == [
        ((1.0, 2.0), 0.0),
        ((5.0, 2.0), 0.0),
        ((5.0, 4.0), 0.0),
        ((1.0, 4.0), 0.0),
    ]
    assert r == CadRectangle([1, 2], (5, 4), 0.0) and hash(r) == hash(CadRectangle((1, 2), (5, 4)))
    assert repr(r).startswith("CadRectangle(first=(1")
    sized = CadRectangle.sized((0, 0), 4.0, 3.0, 0.0, (-1, -1))
    assert sized.other == (-4.0, -3.0)
    assert CadRectangle.of_area((0, 0), 12.0, 3.0, False, 0.0).other == (4.0, 3.0)
    with pytest.raises(InvalidGeometryError):
        CadRectangle((0, 0), (5, 0))
    with pytest.raises(InvalidGeometryError):
        CadRectangle((0, 0), (1, 1), math.nan)
    with pytest.raises(InvalidGeometryError):
        CadRectangle.sized((0, 0), -1.0, 3.0, 0.0, (1, 1))
    with pytest.raises(InvalidGeometryError):
        CadRectangle.of_area((0, 0), 12.0, 0.0, True, 0.0)


def test_rectang_in_spanish() -> None:
    h = Harness()
    set_language("es")
    for typed in ("RECTANG", "REC", "RECTANGLE"):
        found = h.runner.registry.find(typed)
        assert found is not None and found.name == "RECTANG"


def test_rectang_is_in_the_draw_group_menu_and_toolbar(qtbot: QtBot) -> None:
    from PySide6.QtWidgets import QMenu, QToolBar

    from arqstudiocad.ui.ribbon import RibbonGroupWidget

    window = MainWindow(ui=QuietUi())
    qtbot.addWidget(window)
    rectang = window.command_actions["RECTANG"]
    assert rectang.text() == "&Rectangle"
    groups = {g.group.name: g for g in window.findChildren(RibbonGroupWidget)}
    draw = [b.defaultAction() for b in groups["draw"].buttons]
    assert draw[2] == rectang, "right after Polyline"
    menus = {m.title(): m for m in window.menuBar().findChildren(QMenu)}
    assert rectang in menus["&Draw"].actions()
    assert any(rectang in bar.actions() for bar in window.findChildren(QToolBar))


def test_the_viewport_previews_the_rectangle(qtbot: QtBot, tmp_path: Path) -> None:
    from PySide6.QtCore import QEvent
    from test_point_input import mouse, plan

    from arqstudiocad.ui.overlay import Stroke

    ui = QuietUi()
    window = MainWindow(ui=ui)
    ui.view_for = window.viewport_for
    qtbot.addWidget(window)
    window.resize(900, 600)
    window.show()
    qtbot.waitExposed(window)
    viewport = plan(window, tmp_path)
    window.session.drafting.set_osnap(False)

    def preview(drag: Drag, at: tuple[float, float]) -> CadRectangle | None:
        viewport.ask_point((10.0, 20.0), drag)
        mouse(viewport, QEvent.Type.MouseMove, at)
        return viewport.rectangle_preview()

    shown = preview(Drag("rectangle", band=False), (30.0, 25.0))
    assert shown is not None and shown.other == pytest.approx((30.0, 25.0))
    assert viewport.preview is None, "nothing is moved"

    def drawn(rectangle: CadRectangle) -> list[tuple[float, float]]:
        """The overlay's closed stroke inside the rectangle's box."""
        corners = [viewport.screen_point(c) for c in rectangle.corners]
        strokes = [i for i in viewport.overlay().items if isinstance(i, Stroke)]
        found = [s for s in strokes if math.dist(s.points[-1], s.points[0]) < 1e-6]
        on_box = [
            s
            for s in found
            if all(
                min(c.x() for c in corners) - 1e-6 <= x <= max(c.x() for c in corners) + 1e-6
                and min(c.y() for c in corners) - 1e-6 <= y <= max(c.y() for c in corners) + 1e-6
                for x, y in s.points
            )
        ]
        assert on_box, "the rectangle is drawn over the drawing"
        return list(on_box[-1].points)

    corners = [(c.x(), c.y()) for c in (viewport.screen_point(c) for c in shown.corners)]
    points = drawn(shown)
    assert len(points) == 5 and all(
        math.dist(p, c) < 1e-6 for p, c in zip(points, [*corners, corners[0]], strict=True)
    ), "square corners: the four corners, closed"

    # Filleted, the preview rounds the corners: none of them is drawn.
    shown = preview(Drag("rectangle", band=False, fillet=2.0), (30.0, 25.0))
    assert shown is not None
    points = drawn(shown)
    assert len(points) > 9, "arcs drawn as fine polylines"
    for corner in corners:
        assert all(math.dist(p, corner) > 1.0 for p in points), "the corner is rounded"
    # Too small for them: the square rectangle RECTANG will draw.
    shown = preview(Drag("rectangle", band=False, fillet=20.0), (30.0, 25.0))
    assert shown is not None and len(drawn(shown)) == 5
    # Chamfered: eight vertices, closed.
    shown = preview(Drag("rectangle", band=False, chamfer=(2.0, 1.0)), (30.0, 25.0))
    assert shown is not None and len(drawn(shown)) == 9

    # Dimensions: the size is kept, the point only picks the quarter.
    shown = preview(Drag("rectangle", band=False, size=(4.0, 3.0)), (0.0, 25.0))
    assert shown is not None and shown.other == pytest.approx((6.0, 23.0))
    # Turned a right angle, the length runs up.
    shown = preview(Drag("rectangle", band=False, rotation=math.pi / 2), (5.0, 30.0))
    assert shown is not None and (shown.length, shown.width) == pytest.approx((10.0, 5.0))
    # On a side's line there is no rectangle yet.
    assert preview(Drag("rectangle", band=False), (30.0, 20.0)) is None


def saved_path(
    h: Harness, folder: Path, name: str = "r.dxf"
) -> list[tuple[tuple[float, float], float]]:
    """The last polyline's vertices and bulges as the saved DXF holds them
    (codes 10, 20 and 42 of its LWPOLYLINE), read back by hand."""
    path = folder / name
    h.session.save(h.document, path)
    lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
    pairs = list(zip(lines[0::2], lines[1::2], strict=False))
    vertices: list[list[float]] = []
    inside = False
    for code, value in ((c.strip(), v.strip()) for c, v in pairs):
        if code == "0":
            inside = value == "LWPOLYLINE"
            if inside:
                vertices = []
            continue
        if not inside:
            continue
        if code == "10":
            vertices.append([float(value), 0.0, 0.0])
        elif code == "20":
            vertices[-1][1] = float(value)
        elif code == "42":
            vertices[-1][2] = float(value)
    return [((round(x, 9) + 0.0, round(y, 9) + 0.0), b) for x, y, b in vertices]


def same_path(
    got: list[tuple[tuple[float, float], float]], expected: list[tuple[tuple[float, float], float]]
) -> bool:
    return len(got) == len(expected) and all(
        math.dist(p, q) < 1e-9 and abs(b - c) < 1e-9
        for (p, b), (q, c) in zip(got, expected, strict=True)
    )


def test_fillet_rounds_the_corners(tmp_path: Path) -> None:
    h = Harness()
    h.run("RECTANG", "F")
    assert h.prompt().text == "Specify fillet radius for rectangles <0>:"
    assert not h.prompt().point, "a radius is typed"
    h.run("0.5")
    assert h.prompt().text == FIRST, "then the first corner again"
    h.run("0,0")
    assert h.prompt().drag == Drag("rectangle", band=False, rotation=0.0, fillet=0.5)
    h.run("4,2")
    assert same_path(
        saved_path(h, tmp_path),
        [
            ((0.5, 0.0), 0.0),
            ((3.5, 0.0), Q),
            ((4.0, 0.5), 0.0),
            ((4.0, 1.5), Q),
            ((3.5, 2.0), 0.0),
            ((0.5, 2.0), Q),
            ((0.0, 1.5), 0.0),
            ((0.0, 0.5), Q),
        ],
    )
    assert h.document.undo_history == ["RECTANG"]
    # Remembered: the next rectangle says so and rounds its corners too.
    h.run("RECTANG")
    assert h.messages()[-1] == "Current rectangle modes: Fillet=0.5"
    h.run("10,0", "12,2")
    assert len(saved_path(h, tmp_path)) == 8


def test_chamfer_cuts_the_length_by_the_first_distance(tmp_path: Path) -> None:
    h = Harness()
    h.run("RECTANG", "C")
    assert h.prompt().text == "Specify first chamfer distance for rectangles <0>:"
    h.run("1")
    assert h.prompt().text == "Specify second chamfer distance for rectangles <1>:", (
        "the second defaults to the first"
    )
    h.run("0.5", "0,0", "4,2")
    assert same_path(
        saved_path(h, tmp_path),
        [
            ((1.0, 0.0), 0.0),
            ((3.0, 0.0), 0.0),
            ((4.0, 0.5), 0.0),
            ((4.0, 1.5), 0.0),
            ((3.0, 2.0), 0.0),
            ((1.0, 2.0), 0.0),
            ((0.0, 1.5), 0.0),
            ((0.0, 0.5), 0.0),
        ],
    )
    h.run("RECTANG")
    assert h.messages()[-1] == "Current rectangle modes: Chamfer=1 x 0.5"
    assert h.prompt().text == FIRST


def test_fillet_and_chamfer_replace_each_other_and_zero_squares_them(tmp_path: Path) -> None:
    h = Harness()
    h.run("RECTANG", "F", "0.5", "C", "1", "0.5")
    h.run("0,0", "4,2")
    path = saved_path(h, tmp_path)
    assert len(path) == 8 and all(b == 0.0 for _, b in path), "the chamfer replaced the fillet"
    h.run("RECTANG", "F")
    assert h.prompt().text == "Specify fillet radius for rectangles <0>:", "the fillet was cleared"
    h.run("0", "0,0", "4,2")
    assert same_path(
        saved_path(h, tmp_path),
        [((0.0, 0.0), 0.0), ((4.0, 0.0), 0.0), ((4.0, 2.0), 0.0), ((0.0, 2.0), 0.0)],
    )
    # No modes to report now.
    count = len(h.messages())
    h.run("RECTANG")
    assert len(h.messages()) == count
    h.runner.cancel()


@pytest.mark.parametrize("typed", ["abc", "-1", "nan"])
def test_a_radius_must_be_zero_or_more(typed: str) -> None:
    h = Harness()
    h.run("RECTANG", "F", typed)
    assert h.messages()[-1] == "Type a number zero or greater."
    assert h.prompt().text == "Specify fillet radius for rectangles <0>:"


def test_corners_too_large_leave_the_rectangle_square(tmp_path: Path) -> None:
    h = Harness()
    h.run("RECTANG", "F", "1.5", "0,0", "4,2")
    assert h.messages()[-1] == "The rectangle is too small for its fillets; its corners are square."
    assert same_path(
        saved_path(h, tmp_path),
        [((0.0, 0.0), 0.0), ((4.0, 0.0), 0.0), ((4.0, 2.0), 0.0), ((0.0, 2.0), 0.0)],
    )
    h.run("RECTANG", "C", "3", "0.5", "0,0", "4,2")
    assert (
        h.messages()[-1] == "The rectangle is too small for its chamfers; its corners are square."
    )
    # Exactly half the width: two half circles and two straight sides.
    h.run("RECTANG", "F", "1", "0,0", "4,2")
    path = saved_path(h, tmp_path)
    assert len(path) == 6 and sum(1 for _, b in path if b) == 4


def test_area_counts_what_the_corners_take(tmp_path: Path) -> None:
    h = Harness()
    h.run("RECTANG", "C", "1", "0.5", "0,0", "A", "7", "L", "4")
    # 4 x 2 less four triangles of 1 x 0.5: the area typed.
    path = saved_path(h, tmp_path)
    assert len(path) == 8
    xs, ys = [p[0] for p, _ in path], [p[1] for p, _ in path]
    assert (max(xs) - min(xs), max(ys) - min(ys)) == pytest.approx((4.0, 2.0))
    h.run("RECTANG", "F", "0.5", "0,0", "A", "12", "L", "4")
    rectangle = CadRectangle.of_area((0, 0), 12.0, 4.0, True, 0.0, fillet=0.5)
    assert rectangle.area_with(fillet=0.5) == pytest.approx(12.0)
    assert same_path(saved_path(h, tmp_path), [(p, b) for p, b in rectangle.path(fillet=0.5)])


def test_turned_and_filleted_it_survives_dwg(tmp_path: Path) -> None:
    h = Harness()
    h.run("RECTANG", "F", "0.25", "100000.5,200000.25", "R", "30", "D", "4", "3", "100010,200010")
    expected = saved_path(h, tmp_path, "r.dxf")
    assert len(expected) == 8 and sum(1 for _, b in expected if abs(b - Q) < 1e-12) == 4
    path = tmp_path / "r.dwg"
    h.session.save(h.document, path)
    again = Harness()
    again.document = again.session.open_document(path)
    assert same_path(saved_path(again, tmp_path, "again.dxf"), expected)


def test_core_rectangle_corners() -> None:
    r = CadRectangle((0.0, 0.0), (4.0, 2.0))
    assert r.path() == r.vertices
    assert len(r.path(fillet=0.5)) == 8 and r.fits(fillet=1.0) and not r.fits(fillet=1.01)
    assert r.area_with(chamfer=(1.0, 0.5)) == pytest.approx(7.0)
    assert r.area_with(fillet=1.0) == pytest.approx(8.0 - (4.0 - math.pi))
    with pytest.raises(InvalidGeometryError):
        r.path(fillet=1.5)
    with pytest.raises(InvalidGeometryError):
        r.path(chamfer=(-1.0, 1.0))
    with pytest.raises(ValueError, match="not both"):
        r.path(fillet=0.5, chamfer=(1.0, 1.0))
