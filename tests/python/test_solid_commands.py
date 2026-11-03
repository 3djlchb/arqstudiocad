"""The 3D commands (ADR 0024): MASSPROP reports the solids selected, block
references' too, with every digit that holds, and says why the rest were
not measured; it is in the ribbon's 3D tab and in Tools > Inquiry."""

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
from arqstudiocad.core import CadDocument, CadMassProperties
from arqstudiocad.events import CommandCancelled, CommandFinished
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
