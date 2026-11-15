"""Numbers that are not money, written and read as CAD programs do (ADR
0039): a point for decimals, no thousands separator, whatever the system's
language."""

import pytest
from pytestqt.qtbot import QtBot

from arqstudiocad.ui.block_dialogs import InsertOptionsDialog
from arqstudiocad.ui.blocks_panel import InsertOptions
from arqstudiocad.ui.number_format import format_number, parse_number


@pytest.mark.usefixtures("decimal_comma_locale")
def test_a_decimal_point_and_no_thousands_separator_in_any_language() -> None:
    assert format_number(2.5) == "2.5"
    assert format_number(12345.678) == "12345.678"
    assert format_number(2.0) == "2"
    assert parse_number("2.5") == 2.5
    assert parse_number("-0.25") == -0.25
    for typed in ("2,5", "1,234.5", "1.234,5", "abc", ""):
        assert parse_number(typed) is None, typed


@pytest.mark.usefixtures("decimal_comma_locale")
def test_number_boxes_write_a_decimal_point_in_any_language(qtbot: QtBot) -> None:
    dialog = InsertOptionsDialog(
        "Door", InsertOptions(scale=1.5, rotation=45.25, on_screen=True, explode=False)
    )
    qtbot.addWidget(dialog)
    assert dialog.scale.text() == "1.500"
    assert dialog.rotation.text() == "45.25°"
    assert dialog.scale.valueFromText("2.25") == 2.25
