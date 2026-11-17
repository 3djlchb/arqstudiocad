"""MATCHPROP (docs/design/matchprop-command.md): the source picked, each
selection of destinations one undo step, Settings remembered, locked
targets left, the dialog and the shared action."""

import pytest
from PySide6.QtWidgets import QMenu, QToolBar
from pytestqt.qtbot import QtBot
from test_trim import Harness
from ui_fakes import QuietUi

from arqstudiocad.core import CadApplication, CadColor
from arqstudiocad.ui.main_window import MainWindow
from arqstudiocad.ui.match_settings_dialog import MatchSettingsDialog
from arqstudiocad.ui.ribbon import RibbonGroupWidget

EVERY = (
    "Current active settings: Color Layer Linetype Linetype scale Lineweight"
    " Transparency Thickness Text Polyline"
)


def styled(h: Harness) -> int:
    """A source line on layer A-WALLS, red."""
    layer, _ = h.document.create_layer("A-WALLS")
    source = h.line((0.0, 0.0), (1.0, 0.0))
    h.document.set_entity_layer([source], layer)
    h.document.set_entity_color([source], CadColor.from_index(1))
    return source


def choose(h: Harness, ids: list[int]) -> None:
    h.session.select(h.document, ids)
    h.run("")


def test_each_selection_of_destinations_is_one_undo_step() -> None:
    h = Harness()
    source = styled(h)
    a = h.line((0.0, 2.0), (1.0, 2.0))
    b, _ = h.document.add_circle((5.0, 5.0), 1.0)
    c = h.line((0.0, 4.0), (1.0, 4.0))
    h.run("MA")
    assert h.prompt().text == "Select source object:"
    h.runner.answer_entity(source, (0.5, 0.0))
    assert h.messages()[-1] == EVERY
    assert h.prompt().text == "Select destination object(s) [Settings(S)]:"
    choose(h, [a, b])
    assert h.runner.active is not None, "it asks again"
    choose(h, [c])
    h.run("")
    assert h.runner.active is None
    for target in (a, b, c):
        entity = h.document.get_entity(target)
        assert (entity.layer, entity.color) == ("A-WALLS", CadColor.from_index(1))
    assert h.document.undo_history[-2:] == ["MATCHPROP", "MATCHPROP"]
    h.run("UNDO")
    assert h.document.get_entity(c).layer != "A-WALLS"
    assert h.document.get_entity(a).layer == "A-WALLS"


def test_settings_choose_the_properties_and_are_remembered() -> None:
    h = Harness()
    ui = QuietUi()
    ui.match_settings_answer = frozenset({"color"})
    h.runner.context.ui = ui
    source = styled(h)
    target = h.line((0.0, 2.0), (1.0, 2.0))
    h.run("MATCHPROP")
    h.runner.answer_entity(source, (0.5, 0.0))
    h.run("S")
    assert ui.asked[-1].startswith("match settings color layer")
    assert h.messages()[-1] == "Current active settings: Color"
    choose(h, [target])
    h.run("")
    entity = h.document.get_entity(target)
    assert entity.color == CadColor.from_index(1)
    assert entity.layer != "A-WALLS", "the layer was not chosen"
    h.run("MATCHPROP")
    h.runner.answer_entity(source, (0.5, 0.0))
    assert h.messages()[-1] == "Current active settings: Color", "remembered"
    h.runner.cancel()


def test_locked_targets_are_left_and_cancelling_changes_nothing() -> None:
    h = Harness()
    source = styled(h)
    locked_layer, _ = h.document.create_layer("LOCKED")
    locked = h.line((0.0, 6.0), (1.0, 6.0))
    h.document.set_entity_layer([locked], locked_layer)
    h.document.set_layer_locked(locked_layer, True)
    free = h.line((0.0, 2.0), (1.0, 2.0))
    before = h.document.undo_history
    h.run("MATCHPROP")
    h.runner.answer_entity(source, (0.5, 0.0))
    h.session.select(h.document, [free])
    h.runner.cancel()
    assert h.document.undo_history == before
    assert h.document.get_entity(free).layer != "A-WALLS"
    h.run("MATCHPROP")
    h.runner.answer_entity(source, (0.5, 0.0))
    choose(h, [locked])
    assert h.messages()[-1] == "1 were on a locked layer."
    assert h.document.get_entity(locked).layer == "LOCKED"
    assert h.document.undo_history == before, "nothing changed: no step"


def test_the_settings_dialog_checks_what_is_chosen(qtbot: QtBot) -> None:
    dialog = MatchSettingsDialog({"color", "polyline"})
    qtbot.addWidget(dialog)
    assert set(dialog.boxes) == {
        "color",
        "layer",
        "linetype",
        "ltscale",
        "lineweight",
        "transparency",
        "thickness",
        "text",
        "polyline",
    }
    assert dialog.chosen() == {"color", "polyline"}
    dialog.boxes["layer"].setChecked(True)
    dialog.boxes["color"].setChecked(False)
    assert dialog.chosen() == {"layer", "polyline"}


def test_matchprop_action_is_shared_and_speaks_spanish(qtbot: QtBot) -> None:
    ui = QuietUi()
    window = MainWindow(ui=ui)
    ui.view_for = window.viewport_for
    qtbot.addWidget(window)
    action = window.command_actions["MATCHPROP"]
    groups = {g.group.name: g for g in window.findChildren(RibbonGroupWidget)}
    menus = {m.title(): m for m in window.menuBar().findChildren(QMenu)}
    assert action in menus["&Modify"].actions()
    assert action in [button.defaultAction() for button in groups["properties"].buttons]
    assert any(action in bar.actions() for bar in window.findChildren(QToolBar))
    window.runner.execute("NEW")
    window.change_language("es")
    assert action.text() == "&Igualar propiedades"
    found = window.runner.registry.find("IGUALARPROP")
    assert found is not None and found.name == "MATCHPROP"
    window.runner.execute("IGUALARPROP")
    assert window.runner.prompt is not None
    assert window.runner.prompt.message == "Seleccione el objeto de origen"
    window.runner.cancel()


def test_the_binding_gives_the_named_properties_and_refuses_unknown_ones() -> None:
    document = CadApplication().create_document("matchprop")
    source, _ = document.add_polyline([((0, 0), 0), ((1, 0), 0)], width=0.5, thickness=2.0)
    target, _ = document.add_polyline([((0, 2), 0), ((1, 2), 0)])
    document.set_entity_color([source], CadColor.from_index(5))
    changed, locked, changes = document.match_properties(
        source, [target], ["polyline", "thickness"]
    )
    assert (changed, locked) == (1, 0)
    assert list(changes.modified) == [target]
    details = dict(document.get_entity(target).details)
    assert (details["Constant width"], details["Thickness"]) == ("0.500000", "2.000000")
    assert document.get_entity(target).color != CadColor.from_index(5), "color not named"
    with pytest.raises(ValueError, match="unknown MATCHPROP setting"):
        document.match_properties(source, [target], ["colour"])


def test_the_binding_gives_a_texts_height_by_the_text_setting() -> None:
    document = CadApplication().create_document("matchprop-text")
    source, _ = document.add_text("A-101", (0, 0), 2.5, 0.0)
    target, _ = document.add_text("B-202", (0, 5), 1.0, 0.0)
    changed, _, _ = document.match_properties(source, [target], ["color"])
    assert changed == 0, "the text setting not named: nothing"
    changed, _, _ = document.match_properties(source, [target], ["text"])
    assert changed == 1
    text = document.get_entity(target).text
    assert text is not None and text.height == 2.5 and text.content == "B-202"
