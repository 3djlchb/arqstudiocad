"""The Currencies dialog and CURRENCIES (ADR 0027 step 3, maintainer
2026-10-27): the drawing's register as a table with Active boxes and a
Principal choice, names and symbols edited in place, currencies added from
the ISO 4217 catalog with a search or made by the user."""

import pytest
from PySide6.QtCore import Qt
from PySide6.QtWidgets import QDialog, QDialogButtonBox, QWidget
from pytestqt.qtbot import QtBot
from test_blocks import Harness
from ui_fakes import QuietUi

from arqstudiocad.core import (
    CadApplication,
    CadBlockData,
    CadCurrency,
    CadDocument,
    InvalidOperationError,
)
from arqstudiocad.currencies import (
    CATALOG,
    CurrencyRegister,
    catalog_currency,
    format_amount,
    parse_amount,
    shown_name,
)
from arqstudiocad.events import DocumentChanged
from arqstudiocad.i18n import discover, set_language
from arqstudiocad.ui import currencies_dialog
from arqstudiocad.ui.currencies_dialog import (
    ACTIVE,
    CODE,
    FORMAT,
    FORMATS,
    NAME,
    PRINCIPAL,
    SYMBOL,
    USES,
    CatalogDialog,
    CurrenciesDialog,
    OwnCurrencyDialog,
)
from arqstudiocad.ui.qt_services import QtUiServices

USD = CadCurrency("USD", "US dollar", "$", 2, True)
PEN = CadCurrency("PEN", "Peruvian sol", "S/", 2, True)
EUR = CadCurrency("EUR", "Euro", "€", 2, False)
ANSWER = CurrencyRegister([USD], "USD", " ", ",")


def drawing() -> CadDocument:
    """USD (principal), PEN and EUR (inactive); MESA costs 450 PEN."""
    document = CadApplication().create_document("Plano")
    a, _ = document.add_line((0, 0), (1, 0))
    document.create_block("MESA", (0, 0), [a])
    document.set_currencies([USD, PEN, EUR], "USD")
    document.set_block_data("MESA", CadBlockData(cost="450", currency="PEN"))
    return document


# The catalog


def test_the_catalog_lists_iso_currencies_once_with_their_decimals() -> None:
    codes = [c.code for c in CATALOG]
    assert codes == sorted(codes) and len(set(codes)) == len(codes)
    assert len(CATALOG) >= 150
    assert all(len(c.code) == 3 and c.code.isupper() and c.name for c in CATALOG)
    assert {c.decimals for c in CATALOG} <= {0, 2, 3}
    assert catalog_currency("USD") == ("USD", "US dollar", "$", 2, ",", "."), "the core's default"
    assert catalog_currency("EUR")[4:] == (".", ",")  # type: ignore[index]
    assert catalog_currency("RUB")[4:] == (" ", ",")  # type: ignore[index]
    assert catalog_currency("CHF")[4:] == (" ", ".")  # type: ignore[index]
    assert all((c.thousands, c.decimal) in FORMATS for c in CATALOG)
    assert catalog_currency("JPY").decimals == 0  # type: ignore[union-attr]
    assert catalog_currency("KWD").decimals == 3  # type: ignore[union-attr]
    assert catalog_currency("BGN") is None, "the euro since 2026"
    assert catalog_currency("XAU") is None, "no metals or funds"


def test_every_catalog_name_is_translated() -> None:
    spanish = discover(strict=True)["es"].ui
    missing = [c.name for c in CATALOG if c.name not in spanish]
    assert missing == []
    set_language("es")
    assert shown_name("Peruvian sol") == "Sol peruano"
    assert shown_name("Moneda de la obra") == "Moneda de la obra", "the user's own"


# CURRENCIES


def harness(ui: QuietUi | None = None) -> Harness:
    h = Harness(ui)
    h.document.set_currencies([USD, PEN, EUR], "USD")
    a, _ = h.document.add_line((0, 0), (1, 0))
    h.document.create_block("MESA", (0, 0), [a])
    h.document.set_block_data("MESA", CadBlockData(cost="450", currency="PEN"))
    h.steps = len(h.document.undo_history)
    return h


def test_currencies_writes_what_the_dialog_proposes() -> None:
    ui = QuietUi(currencies_answer=CurrencyRegister([USD, PEN], "PEN", ".", ","))
    h = harness(ui)
    h.run("CURRENCIES")
    assert ui.currencies_shown == [CurrencyRegister([USD, PEN, EUR], "USD", ",", ".")]
    assert h.document.currencies == ([USD, PEN], "PEN")
    assert h.document.number_format == (".", ",")
    assert h.new_steps() == ["CURRENCIES"] and h.runner.active is None
    assert h.messages()[-1] == "Currencies changed."
    changed = [e for e in h.events if isinstance(e, DocumentChanged)]
    assert changed[-1].changes.document_changed, "the interface hears of it"


def test_currencies_cancelled_or_unchanged_changes_nothing() -> None:
    ui = QuietUi()
    h = harness(ui)
    before = h.messages()
    h.run("CURRENCIES")
    assert h.messages() == before, "cancelled quietly"
    ui.currencies_answer = CurrencyRegister([USD, PEN, EUR], "USD", ",", ".")
    h.run("CURRENCIES")
    assert h.new_steps() == [] and h.runner.active is None
    assert "Currencies changed." not in h.messages()


def test_a_register_that_would_break_a_cost_is_refused() -> None:
    ui = QuietUi(currencies_answer=CurrencyRegister([USD, EUR], "USD", ",", "."))
    h = harness(ui)
    h.run("CURRENCIES")
    assert h.messages()[-1].startswith("The currencies were not changed: PEN is used by 1 block")
    assert h.document.currencies == ([USD, PEN, EUR], "USD") and h.new_steps() == []


def test_without_an_interface_currencies_lists_the_register() -> None:
    h = harness()
    h.run("CURRENCIES")
    assert h.messages()[-3:] == [
        "USD — US dollar (principal)",
        "PEN — Peruvian sol",
        "EUR — Euro (inactive)",
    ]


def test_monedas_in_spanish() -> None:
    h = harness()
    set_language("es")
    h.run("MONEDAS")
    assert h.messages()[-3:] == [
        "USD — Dólar estadounidense (principal)",
        "PEN — Sol peruano",
        "EUR — Euro (inactiva)",
    ]


# The dialog


def dialog(qtbot: QtBot, document: CadDocument) -> CurrenciesDialog:
    shown = CurrenciesDialog(document)
    qtbot.addWidget(shown)
    return shown


def cell(shown: CurrenciesDialog, row: int, column: int) -> str:
    item = shown.table.item(row, column)
    assert item is not None
    return item.text()


def enabled(shown: CurrenciesDialog, row: int, column: int) -> bool:
    item = shown.table.item(row, column)
    assert item is not None
    return bool(item.flags() & Qt.ItemFlag.ItemIsEnabled)


def test_the_dialog_shows_the_register(qtbot: QtBot) -> None:
    shown = dialog(qtbot, drawing())
    assert shown.windowTitle() == "Currencies"
    assert shown.codes() == ["USD", "PEN", "EUR"]
    assert [cell(shown, r, USES) for r in range(3)] == ["0", "1", "0"]
    assert cell(shown, 1, SYMBOL) == "S/"
    assert shown.principal_row() == 0
    assert shown.can_be_principal(1)
    assert not shown.can_be_principal(2), "an inactive one cannot be principal"
    shown.set_principal(2)
    assert shown.principal_row() == 0, "refused"
    assert not enabled(shown, 0, ACTIVE), "the principal one stays active"
    assert not enabled(shown, 1, ACTIVE), "a cost uses PEN"
    assert enabled(shown, 2, ACTIVE)
    assert shown.register() == ([USD, PEN, EUR], "USD", ",", "."), "unchanged"


def test_the_principal_and_active_boxes_follow_each_other(qtbot: QtBot) -> None:
    shown = dialog(qtbot, drawing())
    eur = shown.table.item(2, ACTIVE)
    assert eur is not None
    eur.setCheckState(Qt.CheckState.Checked)
    assert shown.can_be_principal(2)
    shown.table.cellClicked.emit(2, PRINCIPAL)
    assert shown.principal_row() == 2, "a click on its circle"
    shown.table.cellClicked.emit(1, CODE)
    assert shown.principal_row() == 2, "a click elsewhere chooses nothing"
    assert not enabled(shown, 2, ACTIVE) and enabled(shown, 0, ACTIVE)
    usd = shown.table.item(0, ACTIVE)
    assert usd is not None
    usd.setCheckState(Qt.CheckState.Unchecked)
    currencies, principal, *_ = shown.register()
    assert principal == "EUR"
    assert [c.is_active for c in currencies] == [False, True, True]


def test_names_and_symbols_are_edited_in_place(qtbot: QtBot) -> None:
    shown = dialog(qtbot, drawing())
    for column, text in ((NAME, "Sol"), (SYMBOL, "S/.")):
        item = shown.table.item(1, column)
        assert item is not None and item.flags() & Qt.ItemFlag.ItemIsEditable
        item.setText(f" {text} ")
    assert not shown.table.item(1, CODE).flags() & Qt.ItemFlag.ItemIsEditable  # type: ignore[union-attr]
    assert shown.register()[0][1] == CadCurrency("PEN", "Sol", "S/.", 2, True)
    ok = shown.buttons.button(QDialogButtonBox.StandardButton.Ok)
    shown.table.item(1, NAME).setText("  ")  # type: ignore[union-attr]
    assert not ok.isEnabled(), "every currency has a name"


def test_a_catalog_name_is_shown_translated_and_kept_in_english(qtbot: QtBot) -> None:
    set_language("es")
    shown = dialog(qtbot, drawing())
    assert shown.windowTitle() == "Monedas"
    assert cell(shown, 0, NAME) == "Dólar estadounidense"
    for which, text in (
        (QDialogButtonBox.StandardButton.Ok, "Aceptar"),
        (QDialogButtonBox.StandardButton.Cancel, "Cancelar"),
    ):
        assert shown.buttons.button(which).text() == text
    assert shown.register() == ([USD, PEN, EUR], "USD", ",", ".")


def test_remove_only_what_nothing_needs(qtbot: QtBot) -> None:
    shown = dialog(qtbot, drawing())
    for row in (0, 1):
        shown.table.selectRow(row)
        assert not shown.remove.isEnabled(), shown.codes()[row]
        shown._remove()
        assert shown.codes() == ["USD", "PEN", "EUR"], "kept even when asked"
    shown.table.selectRow(2)
    assert shown.remove.isEnabled()
    shown.remove.click()
    assert shown.codes() == ["USD", "PEN"]
    assert shown.register() == ([USD, PEN], "USD", ",", ".")


def test_adding_from_the_catalog(qtbot: QtBot, monkeypatch: pytest.MonkeyPatch) -> None:
    catalog = CatalogDialog({"USD", "PEN"})
    qtbot.addWidget(catalog)
    assert "USD" not in catalog.shown() and "EUR" in catalog.shown()
    catalog.search.setText("peso")
    assert catalog.shown()[:3] == ["ARS", "CLP", "COP"]
    assert catalog.chosen() == catalog_currency("ARS"), "the first one found"
    catalog.search.setText("mxn")
    assert catalog.shown() == ["MXN"]
    catalog.search.setText("nada así")
    assert catalog.shown() == [] and catalog.chosen() is None
    ok = catalog.buttons.button(QDialogButtonBox.StandardButton.Ok)
    assert not ok.isEnabled()
    set_language("es")
    spanish = CatalogDialog(set())
    qtbot.addWidget(spanish)
    spanish.search.setText("sol peruano")
    assert spanish.shown() == ["PEN"]

    shown = dialog(qtbot, drawing())

    def pick(self: CatalogDialog) -> int:
        self.search.setText("JPY")
        return QDialog.DialogCode.Accepted.value

    monkeypatch.setattr(CatalogDialog, "exec", pick)
    shown.add_catalog.click()
    assert shown.codes() == ["USD", "PEN", "EUR", "JPY"]
    jpy = CadCurrency("JPY", "Japanese yen", "¥", 0, True, ",", ".")
    assert shown.register()[0][3] == jpy, "in its usual format"


def test_an_own_currency(qtbot: QtBot, monkeypatch: pytest.MonkeyPatch) -> None:
    own = OwnCurrencyDialog({"USD"})
    qtbot.addWidget(own)
    ok = own.buttons.button(QDialogButtonBox.StandardButton.Ok)
    for code, name, problem in (
        ("", "Unidad", "The code is 3 to 10 capital letters or digits."),
        ("UF-1", "Unidad", "The code is 3 to 10 capital letters or digits."),
        ("usd", "Unidad", "The drawing already has USD."),
        ("uf1", " ", "Give the currency a name."),
        ("uf1", "Unidad de fomento", ""),
    ):
        own.code.setText(code)
        own.name.setText(name)
        assert own.problem.text() == problem, code
        assert ok.isEnabled() == (not problem)
    own.symbol.setText("UF")
    own.decimals.setValue(4)
    assert own.currency() == CadCurrency("UF1", "Unidad de fomento", "UF", 4, True)

    shown = dialog(qtbot, drawing())

    def make(self: OwnCurrencyDialog) -> int:
        self.code.setText("OBRA")
        self.name.setText("Moneda de la obra")
        return QDialog.DialogCode.Accepted.value

    monkeypatch.setattr(OwnCurrencyDialog, "exec", make)
    shown.add_own.click()
    assert shown.register()[0][-1] == CadCurrency("OBRA", "Moneda de la obra", "", 2, True)


def test_the_service_shows_the_dialog(qtbot: QtBot, monkeypatch: pytest.MonkeyPatch) -> None:
    parent = QWidget()
    qtbot.addWidget(parent)
    services = QtUiServices(parent)
    monkeypatch.setattr(currencies_dialog, "edit_currencies", lambda document, parent: ANSWER)
    assert services.edit_currencies(drawing()) == ANSWER
    monkeypatch.undo()
    monkeypatch.setattr(CurrenciesDialog, "exec", lambda self: QDialog.DialogCode.Rejected.value)
    assert currencies_dialog.edit_currencies(drawing()) is None
    monkeypatch.setattr(CurrenciesDialog, "exec", lambda self: QDialog.DialogCode.Accepted.value)
    assert currencies_dialog.edit_currencies(drawing()) == ([USD, PEN, EUR], "USD", ",", ".")


# How amounts are written (maintainer, 2026-10-27)


@pytest.mark.parametrize(
    ("thousands", "decimal", "shown"),
    [
        (",", ".", "1,234,567.89"),
        (".", ",", "1.234.567,89"),
        (" ", ",", "1 234 567,89"),
        (" ", ".", "1 234 567.89"),
    ],
)
def test_amounts_are_shown_and_read_in_the_drawings_format(
    thousands: str, decimal: str, shown: str
) -> None:
    assert format_amount("1234567.89", thousands, decimal) == shown
    assert format_amount("120", thousands, decimal) == "120"
    assert format_amount("0.5", thousands, decimal) == f"0{decimal}5"
    assert parse_amount(shown, thousands, decimal) == "1234567.89"
    plain = shown.replace(thousands, "")
    assert parse_amount(f"  {plain} ", thousands, decimal) == "1234567.89", "thousands optional"
    assert parse_amount("7", thousands, decimal) == "7"


def test_what_is_not_an_amount_in_the_format_is_refused() -> None:
    assert parse_amount("1 234,5", " ", ",") == "1234.5"
    assert parse_amount("1\u00a0234,5", " ", ",") == "1234.5", "a no-break space too"
    for typed in (
        "1250.50",
        "1.25,0",
        "12.3456,0",
        "1234.567,5",
        ".5",
        "",
        "1,2,3",
        "1.234,5x",
        "-5",
        "٣",
    ):
        with pytest.raises(ValueError, match=r"is not an amount: write it as 1\.250,50\."):
            parse_amount(typed, ".", ",")
    set_language("es")
    with pytest.raises(ValueError, match=r"^1,5 no es un importe: escríbalo como 1,250.50\.$"):
        parse_amount("1,5", ",", ".")


def test_the_api_keeps_the_format_unless_given() -> None:
    document = drawing()
    assert document.number_format == (",", ".")
    document.set_currencies([USD, PEN, EUR], "USD", ".", ",")
    assert document.number_format == (".", ",")
    document.set_currencies([USD, PEN, EUR], "PEN")
    assert document.number_format == (".", ","), "kept"
    with pytest.raises(InvalidOperationError, match="thousands are separated"):
        document.set_currencies([USD, PEN, EUR], "USD", ",", ",")
    with pytest.raises(ValueError, match="a separator is one character"):
        document.set_currencies([USD, PEN, EUR], "USD", "..", ",")
    document.undo()
    assert document.number_format == (".", ",") and document.currencies[1] == "USD"


def test_the_dialog_chooses_the_format_and_keeps_the_separators_apart(qtbot: QtBot) -> None:
    shown = dialog(qtbot, drawing())
    assert shown.number_format() == (",", ".")
    assert shown.example.text() == "1,234,567.89"
    shown.thousands.setCurrentIndex(1)  # point: the decimal becomes a comma
    assert shown.number_format() == (".", ",")
    assert shown.example.text() == "1.234.567,89"
    shown.thousands.setCurrentIndex(2)  # space
    assert shown.number_format() == (" ", ",") and shown.example.text() == "1 234 567,89"
    shown.decimal.setCurrentIndex(0)  # point, apart from a space
    assert shown.number_format() == (" ", ".")
    shown.thousands.setCurrentIndex(1)
    shown.decimal.setCurrentIndex(1)
    assert shown.number_format() == (".", ",")
    shown.decimal.setCurrentIndex(0)  # point: the thousands become a comma
    assert shown.number_format() == (",", ".")
    shown.thousands.setCurrentIndex(2)
    assert shown.register() == ([USD, PEN, EUR], "USD", " ", ".")
    set_language("es")
    spanish = dialog(qtbot, drawing())
    assert [spanish.thousands.itemText(i) for i in range(3)] == ["Coma (,)", "Punto (.)", "Espacio"]


def test_the_dialog_opens_on_the_drawings_format(qtbot: QtBot) -> None:
    document = drawing()
    document.set_currencies([USD, PEN, EUR], "USD", " ", ",")
    shown = dialog(qtbot, document)
    assert shown.number_format() == (" ", ",") and shown.example.text() == "1 234 567,89"


# A currency's own format (maintainer, 2026-10-27)


def test_a_currency_carries_its_own_format() -> None:
    own = CadCurrency("EUR", "Euro", "€", 2, True, ".", ",")
    assert (own.thousands, own.decimal) == (".", ",")
    assert (USD.thousands, USD.decimal) == (None, None), "the drawing's"
    with pytest.raises(ValueError, match="go together"):
        CadCurrency("EUR", "Euro", thousands=".")
    document = drawing()
    document.set_currencies([USD, PEN, own], "USD", " ", ",")
    register = CurrencyRegister(*document.currencies, *document.number_format)
    assert register.currencies[2] == own
    assert register.format_of("EUR") == (".", ",")
    assert register.format_of("USD") == (" ", ","), "General"
    assert register.format_of("BOB") == (" ", ","), "not in the register"
    with pytest.raises(InvalidOperationError, match="EUR: thousands are separated"):
        document.set_currencies([USD, CadCurrency("EUR", "Euro", "€", 2, True, ",", ",")], "USD")


def test_the_format_column_chooses_general_or_its_own(qtbot: QtBot) -> None:
    document = drawing()
    euro = CadCurrency("EUR", "Euro", "€", 2, False, ".", ",")
    document.set_currencies([USD, PEN, euro], "USD")
    shown = dialog(qtbot, document)
    lists = [shown.format_list(r) for r in range(3)]
    assert [shown.format_list(0).itemText(i) for i in range(5)] == [
        "General (1,250.50)",
        "1,250.50",
        "1.250,50",
        "1 250,50",
        "1 250.50",
    ]
    assert [f.currentIndex() for f in lists] == [0, 0, 2]
    lists[1].setCurrentIndex(3)
    currencies = shown.register().currencies
    assert (currencies[1].thousands, currencies[1].decimal) == (" ", ",")
    assert (currencies[0].thousands, currencies[0].decimal) == (None, None)
    assert (currencies[2].thousands, currencies[2].decimal) == (".", ",")
    lists[2].setCurrentIndex(0)
    assert shown.register().currencies[2].thousands is None, "back to General"
    # General follows the drawing's format chosen below.
    shown.thousands.setCurrentIndex(1)
    assert lists[0].itemText(0) == "General (1.250,50)"
    set_language("es")
    spanish = dialog(qtbot, document)
    assert spanish.format_list(0).itemText(0) == "General (1,250.50)"
    header = spanish.table.horizontalHeaderItem(FORMAT)
    assert header is not None and header.text() == "Formato"
