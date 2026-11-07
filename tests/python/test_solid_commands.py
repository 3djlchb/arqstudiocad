"""The 3D commands (ADR 0024): MASSPROP reports the solids selected, block
references' too, with every digit that holds, and says why the rest were
not measured; it is in the ribbon's 3D tab and in Tools > Inquiry. UNION,
SUBTRACT and INTERSECT combine solids of flat faces (ADR 0032 D5)."""

import math
from pathlib import Path
from types import SimpleNamespace
from typing import cast

import pytest
from PySide6.QtWidgets import QMenu
from pytestqt.qtbot import QtBot
from test_commands import Harness, harness, regression_drawing
from ui_fakes import QuietUi

from arqstudiocad.commands.solids import refusals, report
from arqstudiocad.core import CadColor, CadDocument, CadMassProperties
from arqstudiocad.events import CommandCancelled, CommandFinished, DocumentChanged
from arqstudiocad.i18n import set_language
from arqstudiocad.ui.main_window import MainWindow
from arqstudiocad.ui.ribbon import Ribbon, RibbonGroupWidget

CORPUS = Path(__file__).resolve().parents[2] / "target/corpus/libredwg"
ATMOS = CORPUS / "2007/ATMOS-DC22S.dwg"
EXAMPLE = CORPUS / "example_2000.dwg"
#: ED: a tube of radii 19 and 16, 70 long, along x, around (828, 485, 930);
#: FC: the same tube, lower down.
TUBE, LOWER_TUBE = 0xED, 0xFC


def opened(path: Path) -> tuple[Harness, CadDocument]:
    h = harness()
    h.run("OPEN", str(regression_drawing(path)))
    document = h.session.active
    assert document is not None
    return h, document


def test_massprop_reports_the_selected_solid() -> None:
    h, document = opened(ATMOS)
    h.session.select(document, [TUBE])
    h.run("MASSPROP")
    volume, area = math.pi * 7350, 2 * math.pi * 2555
    # Ten significant digits: those of the closed forms.
    assert h.messages()[-10:] == [
        "----------------   SOLIDS   ----------------",
        "Solids:        1",
        f"Volume:        {volume:.5f}",
        f"Surface area:  {area:.5f}",
        "Bounding box:  X: 793.00000000  --  863.00000000",
        "               Y: 466.00000000  --  504.00000000",
        "               Z: 911.00000000  --  949.00000000",
        "Centroid:      X: 828.00000000",
        "               Y: 485.00000000",
        "               Z: 930.00000000",
    ]
    assert h.of(CommandFinished)[-1].name == "MASSPROP"
    assert h.session.selection(document) == (), "the selection is used up, as ERASE's"


def test_massprop_selects_and_says_what_it_left_out() -> None:
    h, document = opened(ATMOS)
    h.run("MASSPROP", "ALL", "")
    assert h.messages()[-12:-10] == ["60 found", "2 were not solids."]
    assert h.messages()[-9] == "Solids:        58"
    # Each tube is as precise alone, however far apart they are.
    h.run("MASSPROP", str(TUBE), "")
    h.session.select(document, [TUBE, LOWER_TUBE])
    h.run("MASSPROP")
    assert h.messages()[-8] == f"Volume:        {2 * math.pi * 7350:.5f}"


def test_massprop_measures_solids_on_locked_layers() -> None:
    h, document = opened(ATMOS)
    layer = next(layer for layer in document.get_layers() if layer.name == "0")
    document.set_layer_locked(layer.id, True)
    h.session.select(document, [TUBE])
    h.run("MASSPROP")
    assert h.messages()[-9] == "Solids:        1", "an inquiry edits nothing"


def test_massprop_says_why_nothing_was_measured() -> None:
    h, document = opened(EXAMPLE)
    regions = [e.id for e in document.get_entities() if e.kind == "REGION"]
    line, _ = document.add_line((0, 0), (1, 0))
    h.session.select(document, [*regions, line])
    h.run("MASSPROP")
    assert h.messages()[-3:] == [
        "1 were not solids.",
        f"{len(regions)} do not close around a volume.",
        "No solid could be measured.",
    ]
    assert h.of(CommandCancelled)[-1].name == "MASSPROP"


def test_every_reason_the_api_gives_is_said() -> None:
    reasons = {
        "not_a_solid",
        "untranslated",
        "empty",
        "not_closed",
        "unsupported",
        "not_converged",
        "uneven_scale",
    }
    assert set(refusals()) == reasons
    assert all(said(3).startswith("3 ") for said in refusals().values())


def test_coordinates_far_away_show_what_a_double_holds() -> None:
    h, document = opened(ATMOS)
    document.create_block("TUBO", (828.0, 485.0), [TUBE])
    placed, _ = document.insert_block("TUBO", (1e9, 0.0), scale=1.0, rotation=0.0)
    h.session.select(document, [placed])
    h.run("MASSPROP")
    # Fifteen digits: ten of the tube's size would ask for eight decimals.
    assert h.messages()[-6] == "Bounding box:  X: 999999965.00000  --  1000000035.00000"
    assert h.messages()[-3] == "Centroid:      X: 1000000000.00000"


def test_the_report_counts_digits_across_the_solids() -> None:
    # A unit cube near the origin, its centroid's x a rounding error below 0.
    measures = SimpleNamespace(
        volume=1.0,
        area=6.0,
        centroid=(-1e-13, 0.5, 0.5),
        bounds=((-0.5, 0.0, 0.0), (0.5, 1.0, 1.0)),
    )
    lines = report(cast(CadMassProperties, measures), 1)
    assert lines[2:5] == [
        "Volume:        1.000000000",
        "Surface area:  6.000000000",
        "Bounding box:  X: -0.500000000  --  0.500000000",
    ]
    assert lines[7] == "Centroid:      X: 0.000000000", "not -0.000000000"


def test_propfis_reports_in_spanish() -> None:
    h, document = opened(ATMOS)
    set_language("es")
    try:
        h.session.select(document, [TUBE])
        h.run("PROPFIS")
        assert h.messages()[-10:-7] == [
            "----------------   SÓLIDOS   ----------------",
            "Sólidos:             1",
            f"Volumen:             {math.pi * 7350:.5f}",
        ]
        assert h.messages()[-5] == "                     Y: 466.00000000  --  504.00000000"
    finally:
        set_language("en")


def test_massprop_is_in_the_3d_tab_and_the_tools_menu(qtbot: QtBot) -> None:
    window = MainWindow(ui=QuietUi())
    qtbot.addWidget(window)
    ribbon = window.findChildren(Ribbon)[0]
    tabs = [ribbon.tabText(i) for i in range(ribbon.count()) if ribbon.isTabVisible(i)]
    assert tabs == ["Home", "Insert", "Annotate", "3D", "View"]
    groups = {g.group.name: g for g in window.findChildren(RibbonGroupWidget)}
    massprop = window.command_actions["MASSPROP"]
    assert [b.defaultAction() for b in groups["analysis"].buttons] == [massprop]
    assert groups["analysis"].title.text() == "Analysis"
    menus = {m.title(): m for m in window.menuBar().findChildren(QMenu)}
    assert massprop in menus["&Inquiry"].actions()
    assert menus["&Inquiry"].menuAction() in menus["&Tools"].actions()


@pytest.fixture(autouse=True)
def english() -> None:
    set_language("en")


# BOX (ADR 0024 step 4a)


def corners(m: CadMassProperties) -> list[float]:
    """The box around a solid: its low corner, then its high one."""
    low, high = m.bounds
    return [*low, *high]


def boxed(*lines: str) -> tuple[Harness, CadDocument, list[CadMassProperties]]:
    """A new drawing after ``lines``, and each solid's measures."""
    h = harness()
    h.session.new_document()
    h.run(*lines)
    document = h.session.active
    assert document is not None
    solids = [e.id for e in document.get_entities() if e.kind == "3DSOLID"]
    measures = [document.measure_solids([i]).total for i in solids]
    return h, document, [m for m in measures if m is not None]


def test_box_between_two_corners_and_its_height() -> None:
    h, document, (m,) = boxed("BOX", "0,0", "2,3", "4")
    assert h.prompts()[:3] == [
        "Specify first corner [Center(C)]:",
        "Specify other corner [Cube(C)/Length(L)]:",
        "Specify height [2Point(2P)]:",
    ]
    assert m.volume == pytest.approx(24.0) and m.centroid == pytest.approx((1.0, 1.5, 2.0))
    # The height is remembered; down, it is negative.
    h.run("BOX", "10,0", "11,1", "")
    h.run("BOX", "20,0", "21,1", "-2")
    zs = sorted(
        total.centroid[2]
        for e in document.get_entities()
        if (total := document.measure_solids([e.id]).total) is not None
    )
    assert zs == pytest.approx([-1.0, 2.0, 2.0])
    h.run("UNDO")
    assert len([e for e in document.get_entities() if e.kind == "3DSOLID"]) == 2


def test_box_about_its_center_is_centered_in_height_too() -> None:
    _, _, (m,) = boxed("BOX", "C", "5,5", "6,7", "2")
    assert m.volume == pytest.approx(16.0)
    assert corners(m) == pytest.approx([4.0, 3.0, -1.0, 6.0, 7.0, 1.0])


def test_box_as_a_cube_or_by_its_sides() -> None:
    _, _, (cube,) = boxed("BOX", "0,0", "C", "3")
    assert cube.volume == pytest.approx(27.0)
    assert cube.centroid == pytest.approx((1.5, 1.5, 1.5))
    _, _, (sides,) = boxed("BOX", "0,0", "L", "2", "3", "4")
    assert corners(sides) == pytest.approx([0.0, 0.0, 0.0, 2.0, 3.0, 4.0])
    _, _, (two,) = boxed("BOX", "0,0", "2,3", "2P", "0,0", "0,5")
    assert two.bounds[1][2] == pytest.approx(5.0)


def test_box_asks_again_for_what_makes_no_box() -> None:
    h, _, solids = boxed("BOX", "0,0", "0,3", "2,3", "0", "high", "1")
    assert h.messages()[:3] == [
        "The box needs a length and a width.",
        "The height cannot be zero.",
        "The height must be a number.",
    ]
    assert [m.volume for m in solids] == pytest.approx([6.0])


def test_prismarect_in_spanish() -> None:
    set_language("es")
    try:
        h, _, (m,) = boxed("PRISMARECT", "C", "0,0", "Cubo", "2")
        assert h.prompts()[0] == "Precise primera esquina [Centro(C)]:"
    finally:
        set_language("en")
    assert corners(m) == pytest.approx([-1.0, -1.0, -1.0, 1.0, 1.0, 1.0])


def test_box_is_in_the_3d_tab_and_the_draw_menu(qtbot: QtBot) -> None:
    window = MainWindow(ui=QuietUi())
    qtbot.addWidget(window)
    groups = {g.group.name: g for g in window.findChildren(RibbonGroupWidget)}
    box = window.command_actions["BOX"]
    assert groups["modeling"].buttons[0].defaultAction() is box
    titles = [g.title.text() for g in window.findChildren(RibbonGroupWidget)]
    assert titles.index("Modeling") + 2 == titles.index("Analysis")
    menus = {m.title(): m for m in window.menuBar().findChildren(QMenu)}
    assert box in menus["&Modeling"].actions()
    assert menus["&Modeling"].menuAction() in menus["&Draw"].actions()


# WEDGE (ADR 0024 step 4b)


def test_wedge_stands_at_its_first_corner() -> None:
    h, document, (m,) = boxed("WEDGE", "0,0", "4,2", "3")
    assert h.prompts()[:3] == [
        "Specify first corner [Center(C)]:",
        "Specify other corner [Cube(C)/Length(L)]:",
        "Specify height [2Point(2P)]:",
    ]
    assert m.volume == pytest.approx(12.0)
    assert m.centroid == pytest.approx((4 / 3, 1.0, 1.0))
    # Its own height is remembered, apart from BOX's.
    h.run("BOX", "10,0", "11,1", "5", "WE", "20,0", "24,2", "")
    heights = sorted(
        total.bounds[1][2]
        for e in document.get_entities()
        if (total := document.measure_solids([e.id]).total) is not None
    )
    assert heights == pytest.approx([3.0, 3.0, 5.0])


def test_wedge_about_its_center_stands_at_the_corner_picked() -> None:
    _, _, (m,) = boxed("WE", "C", "2,1", "0,0", "3")
    assert m.volume == pytest.approx(12.0)
    assert m.centroid == pytest.approx((4 / 3, 1.0, -0.5))


def test_a_flat_wedge_is_asked_again() -> None:
    h, _, (m,) = boxed("WEDGE", "0,0", "4,0", "4,2", "-3")
    assert h.messages()[0] == "The wedge needs a length and a width."
    assert m.centroid == pytest.approx((4 / 3, 1.0, -1.0))


def test_cuna_in_spanish_with_or_without_its_tilde() -> None:
    set_language("es")
    try:
        for typed in ("CUÑA", "cuna"):
            _, _, (m,) = boxed(typed, "0,0", "Cubo", "2")
            assert m.volume == pytest.approx(4.0), typed
    finally:
        set_language("en")


def test_wedge_is_beside_box(qtbot: QtBot) -> None:
    window = MainWindow(ui=QuietUi())
    qtbot.addWidget(window)
    groups = {g.group.name: g for g in window.findChildren(RibbonGroupWidget)}
    actions = window.command_actions
    first_two = [b.defaultAction() for b in groups["modeling"].buttons][:2]
    assert first_two == [actions["BOX"], actions["WEDGE"]]
    menus = {m.title(): m for m in window.menuBar().findChildren(QMenu)}
    assert menus["&Modeling"].actions()[:2] == [actions["BOX"], actions["WEDGE"]]


# CYLINDER and CONE (ADR 0024 step 4b-2)


def test_cylinder_by_radius_or_diameter() -> None:
    h, document, (m,) = boxed("CYLINDER", "2,3", "1.5", "4")
    assert h.prompts()[:3] == [
        "Specify center point of base:",
        "Specify base radius [Diameter(D)]:",
        "Specify height [2Point(2P)]:",
    ]
    assert m.volume == pytest.approx(math.pi * 1.5**2 * 4)
    assert m.centroid == pytest.approx((2.0, 3.0, 2.0))
    # The last radius and height are offered again; a diameter is halved.
    h.run("CYL", "10,0", "", "")
    h.run("CYLINDER", "20,0", "D", "1", "-2")
    volumes = sorted(
        total.volume
        for e in document.get_entities()
        if (total := document.measure_solids([e.id]).total) is not None
    )
    assert volumes == pytest.approx(sorted([9 * math.pi, 9 * math.pi, 0.5 * math.pi]))


def test_cone_to_a_point_or_a_top_radius() -> None:
    h, _, (pointed,) = boxed("CONE", "0,0", "3", "4")
    assert pointed.volume == pytest.approx(12 * math.pi)
    assert pointed.centroid[2] == pytest.approx(1.0)
    assert h.prompts()[2] == "Specify height [2Point(2P)/Top radius(T)]:"
    h, _, (frustum,) = boxed("CONE", "0,0", "3", "T", "1", "4")
    assert h.prompts()[3] == "Specify top radius <0>:"
    assert frustum.volume == pytest.approx(math.pi * 4 / 3 * (9 + 3 + 1))
    # Wider at the top than at the base, and downward.
    _, _, (flared,) = boxed("CONE", "0,0", "1", "T", "2", "-3")
    assert flared.volume == pytest.approx(7 * math.pi)
    assert corners(flared)[2] == pytest.approx(-3.0)


def test_a_top_radius_must_be_a_number_not_below_zero() -> None:
    h, _, (m,) = boxed("CONE", "0,0", "3", "T", "wide", "-1", "0", "4")
    assert h.messages()[:2] == ["The radius must be a number.", "The radius cannot be negative."]
    assert m.volume == pytest.approx(12 * math.pi)


def test_cilindro_and_cono_in_spanish() -> None:
    set_language("es")
    try:
        h, _, (m,) = boxed("CILINDRO", "0,0", "Diámetro", "2", "1")
        assert h.prompts()[1] == "Precise el radio de la base [Diámetro(D)]:"
        _, _, (c,) = boxed("CONO", "0,0", "1", "Radio superior", "1", "1")
    finally:
        set_language("en")
    assert m.volume == pytest.approx(math.pi)
    assert c.volume == pytest.approx(math.pi), "equal radii: a cylinder"


def test_round_solids_follow_wedge(qtbot: QtBot) -> None:
    window = MainWindow(ui=QuietUi())
    qtbot.addWidget(window)
    groups = {g.group.name: g for g in window.findChildren(RibbonGroupWidget)}
    actions = window.command_actions
    names = ["BOX", "WEDGE", "CYLINDER", "CONE"]
    shown = [b.defaultAction() for b in groups["modeling"].buttons]
    assert shown[:4] == [actions[n] for n in names]
    menus = {m.title(): m for m in window.menuBar().findChildren(QMenu)}
    assert menus["&Modeling"].actions()[:4] == [actions[n] for n in names]


# SPHERE and TORUS (ADR 0024 step 4b-3)


def test_sphere_by_radius_or_diameter() -> None:
    h, document, (m,) = boxed("SPHERE", "1,2", "2")
    assert h.prompts()[:2] == ["Specify center point:", "Specify radius [Diameter(D)]:"]
    assert m.volume == pytest.approx(32 / 3 * math.pi)
    assert m.centroid == pytest.approx((1.0, 2.0, 0.0)), "its center on the plane"
    h.run("SPHERE", "10,0", "", "SPHERE", "20,0", "D", "2")
    volumes = sorted(
        total.volume
        for e in document.get_entities()
        if (total := document.measure_solids([e.id]).total) is not None
    )
    assert volumes == pytest.approx([4 / 3 * math.pi, 32 / 3 * math.pi, 32 / 3 * math.pi])


def test_torus_with_its_tube() -> None:
    h, _, (m,) = boxed("TORUS", "0,0", "3", "1")
    assert h.prompts()[:3] == [
        "Specify center point:",
        "Specify radius [Diameter(D)]:",
        "Specify tube radius [Diameter(D)]:",
    ]
    assert m.volume == pytest.approx(6 * math.pi**2)
    assert corners(m) == pytest.approx([-4.0, -4.0, -1.0, 4.0, 4.0, 1.0])
    # A tube as thick as the ring is wide is asked again; by diameter too.
    h, _, (thin,) = boxed("TOR", "0,0", "D", "4", "2", "D", "1")
    assert h.messages()[0] == "The tube must be thinner than the torus's radius."
    assert thin.volume == pytest.approx(2 * math.pi**2 * 2 * 0.25)


def test_esfera_and_toroide_in_spanish() -> None:
    set_language("es")
    try:
        h, _, (ball,) = boxed("ESFERA", "0,0", "1")
        assert h.prompts()[1] == "Precise el radio [Diámetro(D)]:"
        _, _, (ring,) = boxed("TOROIDE", "0,0", "2", "Diámetro", "1")
    finally:
        set_language("en")
    assert ball.volume == pytest.approx(4 / 3 * math.pi)
    assert ring.volume == pytest.approx(2 * math.pi**2 * 2 * 0.25)


def test_all_six_primitives_in_the_modeling_group(qtbot: QtBot) -> None:
    window = MainWindow(ui=QuietUi())
    qtbot.addWidget(window)
    groups = {g.group.name: g for g in window.findChildren(RibbonGroupWidget)}
    actions = window.command_actions
    names = ["BOX", "WEDGE", "CYLINDER", "CONE", "SPHERE", "TORUS", "EXTRUDE", "REVOLVE"]
    assert [b.defaultAction() for b in groups["modeling"].buttons] == [actions[n] for n in names]
    menus = {m.title(): m for m in window.menuBar().findChildren(QMenu)}
    assert menus["&Modeling"].actions() == [actions[n] for n in names]


# EXTRUDE (ADR 0024 step 4c)


def profiles() -> tuple[Harness, CadDocument, list[int]]:
    """A drawing with a closed 4 x 2 polyline, a slot (two half circles),
    a circle of radius 1, an open polyline and a line."""
    h = harness()
    h.session.new_document()
    document = h.session.active
    assert document is not None
    square, _ = document.add_polyline([((0, 0), 0), ((4, 0), 0), ((4, 2), 0), ((0, 2), 0)], True)
    slot, _ = document.add_polyline(
        [((10, -1), 0), ((14, -1), 1), ((14, 1), 0), ((10, 1), 1)], closed=True
    )
    circle, _ = document.add_circle((20, 0), 1)
    open_, _ = document.add_polyline([((30, 0), 0), ((32, 0), 0), ((32, 2), 0)])
    line, _ = document.add_line((40, 0), (41, 0))
    return h, document, [square, slot, circle, open_, line]


def solids_of(document: CadDocument) -> list[CadMassProperties]:
    found = (document.measure_solids([e.id]).total for e in document.get_entities())
    return sorted((m for m in found if m is not None), key=lambda m: m.centroid[0])


def test_extrude_makes_solids_of_closed_profiles() -> None:
    h, document, ids = profiles()
    h.session.select(document, ids)
    h.run("EXTRUDE", "3")
    assert h.prompts()[-1] == "Specify height [2Point(2P)]:"
    assert h.messages()[-2:] == [
        "1 were not closed polylines or circles.",
        "1 polylines were not closed.",
    ]
    square, slot, cylinder = solids_of(document)
    assert square.volume == pytest.approx(24.0)
    assert slot.volume == pytest.approx((8 + math.pi) * 3)
    assert cylinder.volume == pytest.approx(3 * math.pi)
    # The closed profiles are erased; the rest stay, and one undo puts back
    # what was.
    kinds = sorted(e.kind for e in document.get_entities())
    assert kinds == ["3DSOLID", "3DSOLID", "3DSOLID", "LINE", "LWPOLYLINE"]
    assert h.session.selection(document) == ()
    h.run("UNDO")
    kinds = sorted(e.kind for e in document.get_entities())
    assert kinds == ["CIRCLE", "LINE", "LWPOLYLINE", "LWPOLYLINE", "LWPOLYLINE"]


def test_extrude_selects_then_asks_the_height_down_or_by_two_points() -> None:
    h, document, ids = profiles()
    h.run("EXT")
    assert h.prompts()[-1] == "Select objects [All/Last(L)/Previous(P)]:"
    h.session.select(document, [ids[0]])
    h.run("", "-2")
    (box,) = solids_of(document)
    assert corners(box)[2] == pytest.approx(-2.0)
    h.session.select(document, [ids[2]])
    h.run("EXTRUDE", "2P", "0,0", "0,5")
    _, cylinder = solids_of(document)
    assert corners(cylinder)[5] == pytest.approx(5.0)
    # The last height is offered again.
    h.session.select(document, [ids[1]])
    h.run("EXTRUDE", "")
    assert solids_of(document)[1].volume == pytest.approx((8 + math.pi) * 5)


def test_extrude_with_nothing_to_extrude_says_so() -> None:
    h, document, ids = profiles()
    h.session.select(document, [ids[4]])
    h.run("EXTRUDE", "1")
    assert h.messages()[-2:] == [
        "1 were not closed polylines or circles.",
        "Nothing could be extruded.",
    ]
    assert h.of(CommandCancelled)[-1].name == "EXTRUDE"
    assert len(document.get_entities()) == 5


def test_extrusion_in_spanish() -> None:
    h, document, ids = profiles()
    set_language("es")
    try:
        h.session.select(document, [ids[0]])
        h.run("EXTRUSION", "1")
        assert h.prompts()[-1] == "Precise altura [2Puntos(2P)]:"
    finally:
        set_language("en")
    assert solids_of(document)[0].volume == pytest.approx(8.0)


def test_every_extrusion_reason_is_said() -> None:
    from arqstudiocad.commands.solids import extrusion_refusals

    assert set(extrusion_refusals()) == {"not_a_profile", "open", "tilted", "crossing", "flat"}


# REVOLVE (ADR 0024 step 4d)


def revolvable() -> tuple[Harness, CadDocument, list[int]]:
    """A 1 x 1 square from x 1 to 2, a square across the Y axis, a circle
    of radius 1 round (3, 10), and a line along Y from (0, 20)."""
    h = harness()
    h.session.new_document()
    document = h.session.active
    assert document is not None
    square, _ = document.add_polyline([((1, 0), 0), ((2, 0), 0), ((2, 1), 0), ((1, 1), 0)], True)
    across, _ = document.add_polyline(
        [((-1, 5), 0), ((1, 5), 0), ((1, 6), 0), ((-1, 6), 0)], closed=True
    )
    circle, _ = document.add_circle((3, 10), 1)
    line, _ = document.add_line((0, 20), (0, 30))
    return h, document, [square, across, circle, line]


def test_revolve_turns_a_profile_about_two_points() -> None:
    h, document, ids = revolvable()
    h.session.select(document, [ids[0]])
    h.run("REVOLVE", "0,0", "0,1", "")
    assert h.prompts()[-3:] == [
        "Specify axis start point [Object(O)/X/Y]:",
        "Specify axis endpoint:",
        "Specify angle of revolution <360>:",
    ]
    (tube,) = solids_of(document)
    assert tube.volume == pytest.approx(3 * math.pi)
    assert corners(tube)[2] == pytest.approx(-2.0) and corners(tube)[5] == pytest.approx(2.0)
    h.run("UNDO")
    assert sorted(e.kind for e in document.get_entities()).count("3DSOLID") == 0


def test_revolve_about_an_axis_or_a_line_by_part_of_a_turn() -> None:
    h, document, ids = revolvable()
    # The circle about the Y axis by its option: a torus 3 round, at y 10.
    h.session.select(document, [ids[2]])
    h.run("REV", "Y", "360")
    # The square about the line, a quarter turn back: x 1 to 2 from it.
    h.session.select(document, [ids[0]])
    h.run("REVOLVE", "O")
    assert h.prompts()[-1] == "Select a line for the axis:"
    # Not a line, or two: asked again.
    h.session.select(document, [ids[1]])
    h.run("")
    assert h.messages()[-1] == "Select one line."
    other, _ = document.add_line((5, 0), (5, 1))
    h.session.select(document, [ids[3], other])
    h.run("")
    assert h.messages()[-2:] == ["Select one line.", "Select one line."]
    h.session.select(document, [ids[3]])
    h.run("", "-90")
    torus, quarter = sorted(solids_of(document), key=lambda m: m.volume, reverse=True)
    assert torus.volume == pytest.approx(6 * math.pi**2)
    assert torus.centroid[1] == pytest.approx(10.0)
    assert quarter.volume == pytest.approx(3 * math.pi / 4)
    assert corners(quarter)[2] == pytest.approx(0.0), "turned back: above the plan"
    assert h.prompts()[-1] == "Specify angle of revolution <360>:"
    # The last angle is offered again.
    h.session.select(document, [ids[1]])
    h.run("REVOLVE", "X")
    assert h.prompts()[-1] == "Specify angle of revolution <-90>:"
    # About X the square across Y, 5 to 6 from it, 2 long: a quarter tube.
    h.run("")
    (across,) = [m for m in solids_of(document) if m.volume not in (torus.volume, quarter.volume)]
    assert across.volume == pytest.approx(22 * math.pi / 4)


def test_revolve_says_why_it_left_profiles() -> None:
    h, document, ids = revolvable()
    h.session.select(document, [ids[1], ids[3]])
    h.run("REVOLVE", "0,0", "0,1", "180")
    assert h.messages()[-3:] == [
        "1 were not closed polylines or circles.",
        "1 reach across the axis.",
        "Nothing could be revolved.",
    ]
    h.session.select(document, [ids[0]])
    h.run("REVOLVE", "0,0", "0,0", "0,1", "cero", "0", "45")
    assert h.messages()[-3:] == [
        "The axis needs two different points.",
        "The angle must be a number.",
        "The angle cannot be zero.",
    ]
    (eighth,) = solids_of(document)
    assert eighth.volume == pytest.approx(3 * math.pi / 8)


def test_revolucion_in_spanish() -> None:
    h, document, ids = revolvable()
    set_language("es")
    try:
        h.session.select(document, [ids[0]])
        h.run("REVOLUCION", "Y")
        assert h.prompts()[-1] == "Precise el ángulo de revolución <360>:"
        h.run("")
    finally:
        set_language("en")
    assert solids_of(document)[0].volume == pytest.approx(3 * math.pi)


def test_every_revolution_reason_is_said() -> None:
    from arqstudiocad.commands.solids import revolution_refusals

    assert set(revolution_refusals()) == {
        "not_a_profile",
        "open",
        "tilted",
        "crossing",
        "flat",
        "crosses_axis",
        "self_crossing_torus",
    }


# Solids moved, copied, turned, scaled and mirrored (ADR 0024 step 5a)


def test_solids_move_copy_rotate_scale_and_mirror_as_a_whole() -> None:
    # A 2 x 3 x 4 box from the origin: centroid (1, 1.5, 2), volume 24.
    h, document, _ = boxed("BOX", "0,0", "2,3", "4")
    (box,) = [e.id for e in document.get_entities() if e.kind == "3DSOLID"]

    def measure() -> CadMassProperties:
        total = document.measure_solids([box]).total
        assert total is not None
        return total

    h.session.select(document, [box])
    h.run("MOVE", "", "0,0", "10,0")
    assert measure().centroid == pytest.approx((11.0, 1.5, 2.0))
    h.session.select(document, [box])
    h.run("ROTATE", "", "10,0", "90")
    assert measure().centroid == pytest.approx((8.5, 1.0, 2.0))
    h.session.select(document, [box])
    h.run("SCALE", "", "10,0", "2")
    m = measure()
    assert m.volume == pytest.approx(192.0)
    assert m.centroid == pytest.approx((7.0, 2.0, 4.0)), "taller too"
    h.session.select(document, [box])
    h.run("MIRROR", "", "0,0", "0,1", "Y")
    assert measure().centroid == pytest.approx((-7.0, 2.0, 4.0))
    assert measure().volume == pytest.approx(192.0)
    h.session.select(document, [box])
    h.run("COPY", "", "0,0", "0,20", "")
    copies = solids_of(document)
    assert len(copies) == 2
    assert sorted(c.centroid[1] for c in copies) == pytest.approx([2.0, 22.0])
    # Each a step of its own: undone back to where the box was made.
    for _ in range(5):
        h.run("UNDO")
    assert measure().centroid == pytest.approx((1.0, 1.5, 2.0))
    assert measure().volume == pytest.approx(24.0)


# UNION, SUBTRACT and INTERSECT (ADR 0032 D5)


def boxes(
    *corners: tuple[tuple[float, float, float], tuple[float, float, float]],
) -> tuple[Harness, CadDocument, list[int]]:
    """A new drawing with a box between each pair of corners, the first on
    layer "MUROS" in red."""
    h = harness()
    h.session.new_document()
    document = h.session.active
    assert document is not None
    ids = [document.add_box(a, b)[0] for a, b in corners]
    layer, _ = document.create_layer("MUROS")
    document.set_entity_layer([ids[0]], layer)
    document.set_entity_color([ids[0]], CadColor.from_index(1))
    return h, document, ids


def solid_ids(document: CadDocument) -> list[int]:
    return [e.id for e in document.get_entities() if e.kind == "3DSOLID"]


def volume_of(document: CadDocument, solid: int) -> float:
    total = document.measure_solids([solid]).total
    assert total is not None
    return total.volume


def test_union_makes_one_solid_of_the_first_chosen() -> None:
    h, document, ids = boxes(((0, 0, 0), (2, 2, 2)), ((1, 1, 1), (3, 3, 3)))
    h.run("UNION")
    assert h.prompts()[-1] == "Select objects [All/Last(L)/Previous(P)]:"
    h.session.select(document, ids)
    h.run("")
    assert h.of(CommandFinished)[-1].name == "UNION"
    # The viewport is told: the second erased, the first changed.
    changed = h.of(DocumentChanged)[-1].changes
    assert (changed.deleted, changed.modified) == ((ids[1],), (ids[0],))
    assert solid_ids(document) == [ids[0]]
    assert volume_of(document, ids[0]) == pytest.approx(15.0)
    (kept,) = document.get_entities([ids[0]])
    assert kept.layer == "MUROS"
    assert kept.color == CadColor.from_index(1)
    assert h.session.selection(document) == ()
    # One undo puts both back.
    h.run("UNDO")
    assert solid_ids(document) == ids
    assert volume_of(document, ids[0]) == pytest.approx(8.0)


def test_union_leaves_out_what_is_not_a_solid() -> None:
    h, document, ids = boxes(((0, 0, 0), (1, 1, 1)), ((5, 0, 0), (6, 1, 1)))
    line, _ = document.add_line((10, 0), (11, 0))
    h.session.select(document, [*ids, line])
    h.run("UNI")
    assert h.messages()[-1] == "1 were not solids."
    # Apart: one solid of two pieces.
    assert solid_ids(document) == [ids[0]]
    assert volume_of(document, ids[0]) == pytest.approx(2.0)
    assert [e.kind for e in document.get_entities([line])] == ["LINE"]


def test_union_of_one_solid_changes_nothing() -> None:
    h, document, ids = boxes(((0, 0, 0), (1, 1, 1)))
    h.session.select(document, ids)
    h.run("UNION")
    assert h.messages()[-2:] == ["Select at least two solids.", "Nothing changed."]
    assert h.of(CommandCancelled)
    assert solid_ids(document) == ids


def test_subtract_asks_what_to_subtract_from_then_what_to_subtract() -> None:
    # A wall of two boxes less two openings, chosen after the command.
    h, document, ids = boxes(
        ((0, 0, 0), (3, 0.2, 2.5)),
        ((3, 0, 0), (6, 0.2, 2.5)),
        ((1, 0, 0), (2, 0.2, 2)),
        ((4, 0, 1), (5, 0.2, 2)),
    )
    h.run("SUBTRACT")
    assert h.messages()[-1] == "Select solids to subtract from…"
    assert h.prompts()[-1] == "Select objects [All/Last(L)/Previous(P)]:"
    h.session.select(document, ids[:2])
    h.run("")
    assert h.messages()[-1] == "Select solids to subtract…"
    assert h.prompts()[-1] == "Select objects [All/Last(L)/Previous(P)]:"
    h.session.select(document, ids[2:])
    h.run("")
    assert h.of(CommandFinished)[-1].name == "SUBTRACT"
    assert solid_ids(document) == [ids[0]]
    assert volume_of(document, ids[0]) == pytest.approx(6 * 0.2 * 2.5 - 0.4 - 0.2)


def test_subtract_with_a_preselection_takes_it_as_what_to_subtract_from() -> None:
    h, document, ids = boxes(((0, 0, 0), (2, 2, 2)), ((1, 1, 1), (3, 3, 3)))
    h.session.select(document, ids[:1])
    h.run("SU")
    assert h.messages()[-1] == "Select solids to subtract…"
    h.session.select(document, ids[1:])
    h.run("")
    assert volume_of(document, ids[0]) == pytest.approx(7.0)


def test_subtract_leaving_nothing_changes_nothing() -> None:
    h, document, ids = boxes(((0, 0, 0), (1, 1, 1)), ((-1, -1, -1), (2, 2, 2)))
    h.session.select(document, ids[:1])
    h.run("SUBTRACT")
    h.session.select(document, ids[1:])
    h.run("")
    assert h.messages()[-2:] == ["Nothing would be left.", "Nothing changed."]
    assert solid_ids(document) == ids


def test_intersect_keeps_what_they_share() -> None:
    h, document, ids = boxes(((0, 0, 0), (2, 2, 2)), ((1, 1, 1), (3, 3, 3)))
    h.session.select(document, ids)
    h.run("IN")
    assert solid_ids(document) == [ids[0]]
    assert volume_of(document, ids[0]) == pytest.approx(1.0)


def test_intersect_of_solids_apart_changes_nothing() -> None:
    h, document, ids = boxes(((0, 0, 0), (1, 1, 1)), ((5, 0, 0), (6, 1, 1)))
    h.session.select(document, ids)
    h.run("INTERSECT")
    assert h.messages()[-2:] == ["The solids do not overlap.", "Nothing changed."]
    assert solid_ids(document) == ids


def test_a_curved_solid_changes_nothing() -> None:
    h, document, ids = boxes(((0, 0, 0), (2, 2, 2)))
    column, _ = document.add_cone((1, 1, -1), 0.5, 0.5, 4)
    h.session.select(document, [*ids, column])
    h.run("UNION")
    assert h.messages()[-2:] == [
        "1 solids have curved faces: only flat ones combine yet.",
        "Nothing changed.",
    ]
    assert solid_ids(document) == [*ids, column]


def test_combining_in_spanish() -> None:
    set_language("es")
    try:
        h, document, ids = boxes(((0, 0, 0), (2, 2, 2)), ((1, 1, 1), (3, 3, 3)))
        h.session.select(document, ids[:1])
        h.run("DIFERENCIA")
        assert h.messages()[-1] == "Seleccione los sólidos que restar…"
        h.session.select(document, ids[1:])
        h.run("")
        assert volume_of(document, ids[0]) == pytest.approx(7.0)
        h2, document2, ids2 = boxes(((0, 0, 0), (1, 1, 1)), ((5, 0, 0), (6, 1, 1)))
        h2.session.select(document2, ids2)
        h2.run("INTERSEC")
        assert h2.messages()[-2:] == ["Los sólidos no se superponen.", "No se cambió nada."]
        h2.session.select(document2, ids2)
        h2.run("UNION")
        assert solid_ids(document2) == [ids2[0]]
    finally:
        set_language("en")


def test_combining_is_in_the_3d_tab_and_the_modify_menu(qtbot: QtBot) -> None:
    window = MainWindow(ui=QuietUi())
    qtbot.addWidget(window)
    groups = {g.group.name: g for g in window.findChildren(RibbonGroupWidget)}
    names = ("UNION", "SUBTRACT", "INTERSECT")
    actions = [window.command_actions[n] for n in names]
    assert [b.defaultAction() for b in groups["solid_editing"].buttons] == actions
    assert groups["solid_editing"].title.text() == "Solid Editing"
    titles = [g.title.text() for g in window.findChildren(RibbonGroupWidget)]
    at = titles.index("Modeling")
    assert titles[at : at + 3] == ["Modeling", "Solid Editing", "Analysis"]
    menus = {m.title(): m for m in window.menuBar().findChildren(QMenu)}
    assert menus["&Solid Editing"].actions() == actions
    assert menus["&Solid Editing"].menuAction() in menus["&Modify"].actions()
