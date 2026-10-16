"""Shared indexed and true-color selection controls."""

import pytest
from PySide6.QtGui import QColor
from pytestqt.qtbot import QtBot

from arqstudiocad.core import CadColor
from arqstudiocad.ui import color_picker, layers_panel
from arqstudiocad.ui.color_picker import ColorPickerDialog, matching_index_colors
from arqstudiocad.ui.properties_panel import _PICK_COLOR, _color_choices


def test_index_search_matches_names_numbers_and_rgb_values() -> None:
    assert matching_index_colors("") == tuple(range(1, 256))
    assert matching_index_colors(color_picker.color_name(CadColor.from_index(5))) == (5,)
    assert matching_index_colors("255") == (255,)
    assert 5 in matching_index_colors("0,0,255")


def test_picker_groups_the_full_aci_palette_and_filters_it(qtbot: QtBot) -> None:
    picker = ColorPickerDialog(CadColor.from_index(7))
    qtbot.addWidget(picker)

    assert set(picker._index_buttons) == set(range(1, 256))
    assert picker.tabs.count() == 2
    assert picker.tabs.tabText(0) == "Index Color"
    assert picker.tabs.tabText(1) == "True Color"

    picker.search.setText("255")
    assert [index for index, button in picker._index_buttons.items() if not button.isHidden()] == [
        255
    ]
    assert picker.selected_color() == CadColor.from_index(7)


def test_picker_preserves_aci_and_true_color_values(qtbot: QtBot) -> None:
    picker = ColorPickerDialog(CadColor.from_index(3))
    qtbot.addWidget(picker)

    picker._select(CadColor.from_index(42))
    assert picker.selected_color() == CadColor.from_index(42)
    picker._on_true_color_changed(QColor(17, 33, 65))
    assert picker.selected_color() == CadColor.from_rgb(17, 33, 65)


def test_recent_colors_are_session_scoped_and_bounded(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(color_picker, "_recent_colors", [])
    colors = [CadColor.from_rgb(index, index + 1, index + 2) for index in range(10)]
    for color in colors:
        color_picker.remember_color(color)

    assert color_picker.recent_colors() == tuple(reversed(colors[-8:]))


def test_properties_color_list_stops_at_light_gray_then_shows_recents(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    recent = [CadColor.from_index(42), CadColor.from_rgb(20, 40, 60)]
    monkeypatch.setattr(color_picker, "_recent_colors", recent)

    choices = _color_choices(CadColor.from_index(42))
    colors = [choice.value for choice in choices if isinstance(choice.value, CadColor)]

    assert [color.index for color in colors if color.index is not None] == [
        *list(range(1, 10)),
        42,
    ]
    assert CadColor.from_rgb(20, 40, 60) in colors
    assert CadColor.from_index(10) not in colors
    assert choices[-1].value == _PICK_COLOR

    layer_menu_colors = layers_panel._quick_color_choices()
    assert [color.index for color in layer_menu_colors if color.index is not None] == [
        *list(range(1, 10)),
        42,
    ]
    assert CadColor.from_rgb(20, 40, 60) in layer_menu_colors
    assert CadColor.from_index(10) not in layer_menu_colors


def test_color_picker_opens_within_available_screen_geometry(qtbot: QtBot) -> None:
    picker = ColorPickerDialog(CadColor.from_index(7))
    qtbot.addWidget(picker)
    screen = picker.screen()
    assert screen is not None
    available = screen.availableGeometry()

    assert picker.width() <= available.width() - 32
    assert picker.height() <= available.height() - 48
