"""Block data for budgets and the drawing's currencies through the Python
API (ADR 0027, step 2: the core). The dialogs and commands come later."""

from pathlib import Path

import pytest

from arqstudiocad.core import (
    CadApplication,
    CadBlockData,
    CadCurrency,
    CadDocument,
    CadError,
    InvalidOperationError,
)


def drawing() -> CadDocument:
    """MESA placed once; SILLA placed nowhere."""
    document = CadApplication().create_document("Plano")
    a, _ = document.add_line((0, 0), (1, 0))
    chair, _ = document.create_block("SILLA", (0, 0), [a])
    document.delete_entities([chair])
    b, _ = document.add_line((0, 0), (2, 0))
    document.create_block("MESA", (0, 0), [b])
    return document


USD = CadCurrency("USD", "US dollar", "$", 2, True)
PEN = CadCurrency("PEN", "Peruvian sol", "S/", 2, True)
TABLE = CadBlockData(
    category="Mobiliario",
    type="Mesa",
    material="Roble",
    supplier="Carpintería Sur",
    cost="120.5",
    currency="USD",
)


def test_values_hold_what_they_were_given() -> None:
    assert TABLE.category == "Mobiliario" and TABLE.type == "Mesa"
    assert (TABLE.material, TABLE.supplier) == ("Roble", "Carpintería Sur")
    assert (TABLE.cost, TABLE.currency) == ("120.5", "USD")
    empty = CadBlockData()
    assert empty == CadBlockData(category="") and empty.cost is None and empty.currency is None
    with pytest.raises(ValueError, match="a cost and its currency go together"):
        CadBlockData(cost="1")
    with pytest.raises(ValueError, match="go together"):
        CadBlockData(currency="USD")
    assert (PEN.code, PEN.name, PEN.symbol, PEN.decimals, PEN.is_active) == (
        "PEN",
        "Peruvian sol",
        "S/",
        2,
        True,
    )
    assert CadCurrency("UF", "Unidad", decimals=4, active=False).symbol == ""


def test_a_new_drawing_has_us_dollars() -> None:
    document = drawing()
    assert document.currencies == ([USD], "USD")
    assert document.currency_uses == {}
    assert document.block_data("MESA") is None and document.blocks_data == {}


def test_block_data_is_set_read_undone_and_removed() -> None:
    document = drawing()
    changes = document.set_block_data("mesa", TABLE)
    assert changes.blocks_modified == ("MESA",)
    assert document.undo_name == "BLOCKDATA"
    stored = document.block_data("Mesa")
    assert stored is not None and stored.cost == "120.50", "with the currency's decimals"
    assert stored.category == "Mobiliario" and stored.type == "Mesa"
    assert document.blocks_data == {"MESA": stored}
    assert document.currency_uses == {"USD": 1}
    document.undo()
    assert document.block_data("MESA") is None
    document.redo()
    document.set_block_data("MESA", CadBlockData())
    assert document.block_data("MESA") is None


def test_what_cannot_be_written_is_refused() -> None:
    document = drawing()
    steps = len(document.undo_history)
    with pytest.raises(InvalidOperationError, match='no block named "NADA"'):
        document.set_block_data("NADA", TABLE)
    with pytest.raises(InvalidOperationError, match="PEN is not an active currency"):
        document.set_block_data("MESA", CadBlockData(cost="1.5", currency="PEN"))
    with pytest.raises(InvalidOperationError, match="more than 2 decimals"):
        document.set_block_data("MESA", CadBlockData(cost="1.234", currency="USD"))
    with pytest.raises(InvalidOperationError, match="is not an amount"):
        document.set_block_data("MESA", CadBlockData(cost="1,5", currency="USD"))
    with pytest.raises(InvalidOperationError, match="the category is one line"):
        document.set_block_data("MESA", CadBlockData(category="a\nb"))
    assert document.block_data("MESA") is None
    assert len(document.undo_history) == steps


def test_the_register_is_set_and_guards_the_currencies_in_use() -> None:
    document = drawing()
    changes = document.set_currencies([USD, PEN], "USD")
    assert changes.document_changed and document.undo_name == "CURRENCIES"
    assert document.currencies == ([USD, PEN], "USD")
    document.set_block_data("MESA", CadBlockData(cost="450", currency="PEN"))
    assert document.currency_uses == {"PEN": 1}
    off = CadCurrency("PEN", "Peruvian sol", "S/", 2, False)
    with pytest.raises(InvalidOperationError, match="PEN is used by 1 block"):
        document.set_currencies([USD, off], "USD")
    with pytest.raises(InvalidOperationError, match="PEN is used by 1 block"):
        document.set_currencies([USD], "USD")
    with pytest.raises(InvalidOperationError, match="the principal currency"):
        document.set_currencies([USD, PEN], "EUR")
    with pytest.raises(InvalidOperationError, match="is not a currency code"):
        document.set_currencies([USD, PEN, CadCurrency("eu", "Euro")], "USD")
    document.set_currencies([PEN], "PEN")
    assert document.currencies == ([PEN], "PEN")
    document.undo()
    assert document.currencies == ([USD, PEN], "USD")


def test_block_data_is_saved_and_read_back(tmp_path: Path) -> None:
    app = CadApplication()
    document = drawing()
    document.set_currencies([USD, PEN], "PEN")
    document.set_block_data("MESA", TABLE)
    document.set_block_data("SILLA", CadBlockData(supplier="Maderera", cost="80", currency="PEN"))
    for name in ("data.dwg", "data.dxf"):
        path = tmp_path / name
        document.save(path)
        back = app.open_document(path)
        assert back.currencies == ([USD, PEN], "PEN"), name
        assert back.blocks_data == document.blocks_data, name
        silla = back.block_data("SILLA")
        assert silla is not None and (silla.cost, silla.currency) == ("80.00", "PEN")


def test_errors_are_cad_errors() -> None:
    assert issubclass(InvalidOperationError, CadError)
