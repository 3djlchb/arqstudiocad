"""The ISO 4217 catalog the Currencies dialog adds from (ADR 0027 D3), and
how a drawing writes amounts: shown with its separators, read back from
what the user types.

The currencies in use today, without funds, precious metals or testing
codes: each one's code, its English name, a common symbol and its minor
units (decimals), as ISO 4217 lists them. These are facts, compiled for
this project. A currency added from here keeps its English name in the
drawing, so the name is shown in the reader's language through ``tr()``.
"""

import re
from typing import NamedTuple

from arqstudiocad.core import CadCurrency
from arqstudiocad.i18n import tr


class CatalogCurrency(NamedTuple):
    """A currency of the catalog."""

    code: str
    name: str
    symbol: str
    decimals: int


#: ISO 4217's current currencies, by code.
CATALOG: tuple[CatalogCurrency, ...] = (
    CatalogCurrency("AED", "UAE dirham", "د.إ", 2),
    CatalogCurrency("AFN", "Afghan afghani", "؋", 2),
    CatalogCurrency("ALL", "Albanian lek", "L", 2),
    CatalogCurrency("AMD", "Armenian dram", "֏", 2),
    CatalogCurrency("AOA", "Angolan kwanza", "Kz", 2),
    CatalogCurrency("ARS", "Argentine peso", "$", 2),
    CatalogCurrency("AUD", "Australian dollar", "$", 2),
    CatalogCurrency("AWG", "Aruban florin", "ƒ", 2),
    CatalogCurrency("AZN", "Azerbaijani manat", "₼", 2),
    CatalogCurrency("BAM", "Bosnia and Herzegovina convertible mark", "KM", 2),
    CatalogCurrency("BBD", "Barbados dollar", "$", 2),
    CatalogCurrency("BDT", "Bangladeshi taka", "৳", 2),
    CatalogCurrency("BHD", "Bahraini dinar", ".د.ب", 3),
    CatalogCurrency("BIF", "Burundian franc", "FBu", 0),
    CatalogCurrency("BMD", "Bermudian dollar", "$", 2),
    CatalogCurrency("BND", "Brunei dollar", "$", 2),
    CatalogCurrency("BOB", "Bolivian boliviano", "Bs", 2),
    CatalogCurrency("BRL", "Brazilian real", "R$", 2),
    CatalogCurrency("BSD", "Bahamian dollar", "$", 2),
    CatalogCurrency("BTN", "Bhutanese ngultrum", "Nu.", 2),
    CatalogCurrency("BWP", "Botswana pula", "P", 2),
    CatalogCurrency("BYN", "Belarusian ruble", "Br", 2),
    CatalogCurrency("BZD", "Belize dollar", "$", 2),
    CatalogCurrency("CAD", "Canadian dollar", "$", 2),
    CatalogCurrency("CDF", "Congolese franc", "FC", 2),
    CatalogCurrency("CHF", "Swiss franc", "CHF", 2),
    CatalogCurrency("CLP", "Chilean peso", "$", 0),
    CatalogCurrency("CNY", "Chinese yuan", "¥", 2),
    CatalogCurrency("COP", "Colombian peso", "$", 2),
    CatalogCurrency("CRC", "Costa Rican colón", "₡", 2),
    CatalogCurrency("CUP", "Cuban peso", "$", 2),
    CatalogCurrency("CVE", "Cape Verdean escudo", "$", 2),
    CatalogCurrency("CZK", "Czech koruna", "Kč", 2),
    CatalogCurrency("DJF", "Djiboutian franc", "Fdj", 0),
    CatalogCurrency("DKK", "Danish krone", "kr", 2),
    CatalogCurrency("DOP", "Dominican peso", "$", 2),
    CatalogCurrency("DZD", "Algerian dinar", "دج", 2),
    CatalogCurrency("EGP", "Egyptian pound", "E£", 2),
    CatalogCurrency("ERN", "Eritrean nakfa", "Nfk", 2),
    CatalogCurrency("ETB", "Ethiopian birr", "Br", 2),
    CatalogCurrency("EUR", "Euro", "€", 2),
    CatalogCurrency("FJD", "Fiji dollar", "$", 2),
    CatalogCurrency("FKP", "Falkland Islands pound", "£", 2),
    CatalogCurrency("GBP", "Pound sterling", "£", 2),
    CatalogCurrency("GEL", "Georgian lari", "₾", 2),
    CatalogCurrency("GHS", "Ghanaian cedi", "₵", 2),
    CatalogCurrency("GIP", "Gibraltar pound", "£", 2),
    CatalogCurrency("GMD", "Gambian dalasi", "D", 2),
    CatalogCurrency("GNF", "Guinean franc", "FG", 0),
    CatalogCurrency("GTQ", "Guatemalan quetzal", "Q", 2),
    CatalogCurrency("GYD", "Guyanese dollar", "$", 2),
    CatalogCurrency("HKD", "Hong Kong dollar", "$", 2),
    CatalogCurrency("HNL", "Honduran lempira", "L", 2),
    CatalogCurrency("HTG", "Haitian gourde", "G", 2),
    CatalogCurrency("HUF", "Hungarian forint", "Ft", 2),
    CatalogCurrency("IDR", "Indonesian rupiah", "Rp", 2),
    CatalogCurrency("ILS", "Israeli new shekel", "₪", 2),
    CatalogCurrency("INR", "Indian rupee", "₹", 2),
    CatalogCurrency("IQD", "Iraqi dinar", "ع.د", 3),
    CatalogCurrency("IRR", "Iranian rial", "﷼", 2),
    CatalogCurrency("ISK", "Icelandic króna", "kr", 0),
    CatalogCurrency("JMD", "Jamaican dollar", "$", 2),
    CatalogCurrency("JOD", "Jordanian dinar", "د.ا", 3),  # noqa: RUF001
    CatalogCurrency("JPY", "Japanese yen", "¥", 0),
    CatalogCurrency("KES", "Kenyan shilling", "KSh", 2),
    CatalogCurrency("KGS", "Kyrgyzstani som", "с", 2),  # noqa: RUF001
    CatalogCurrency("KHR", "Cambodian riel", "៛", 2),
    CatalogCurrency("KMF", "Comorian franc", "CF", 0),
    CatalogCurrency("KPW", "North Korean won", "₩", 2),
    CatalogCurrency("KRW", "South Korean won", "₩", 0),
    CatalogCurrency("KWD", "Kuwaiti dinar", "د.ك", 3),
    CatalogCurrency("KYD", "Cayman Islands dollar", "$", 2),
    CatalogCurrency("KZT", "Kazakhstani tenge", "₸", 2),
    CatalogCurrency("LAK", "Lao kip", "₭", 2),
    CatalogCurrency("LBP", "Lebanese pound", "ل.ل", 2),
    CatalogCurrency("LKR", "Sri Lankan rupee", "Rs", 2),
    CatalogCurrency("LRD", "Liberian dollar", "$", 2),
    CatalogCurrency("LSL", "Lesotho loti", "L", 2),
    CatalogCurrency("LYD", "Libyan dinar", "ل.د", 3),
    CatalogCurrency("MAD", "Moroccan dirham", "د.م.", 2),
    CatalogCurrency("MDL", "Moldovan leu", "L", 2),
    CatalogCurrency("MGA", "Malagasy ariary", "Ar", 2),
    CatalogCurrency("MKD", "Macedonian denar", "ден", 2),
    CatalogCurrency("MMK", "Myanmar kyat", "K", 2),
    CatalogCurrency("MNT", "Mongolian tögrög", "₮", 2),
    CatalogCurrency("MOP", "Macanese pataca", "MOP$", 2),
    CatalogCurrency("MRU", "Mauritanian ouguiya", "UM", 2),
    CatalogCurrency("MUR", "Mauritian rupee", "Rs", 2),
    CatalogCurrency("MVR", "Maldivian rufiyaa", "Rf", 2),
    CatalogCurrency("MWK", "Malawian kwacha", "MK", 2),
    CatalogCurrency("MXN", "Mexican peso", "$", 2),
    CatalogCurrency("MYR", "Malaysian ringgit", "RM", 2),
    CatalogCurrency("MZN", "Mozambican metical", "MT", 2),
    CatalogCurrency("NAD", "Namibian dollar", "$", 2),
    CatalogCurrency("NGN", "Nigerian naira", "₦", 2),
    CatalogCurrency("NIO", "Nicaraguan córdoba", "C$", 2),
    CatalogCurrency("NOK", "Norwegian krone", "kr", 2),
    CatalogCurrency("NPR", "Nepalese rupee", "Rs", 2),
    CatalogCurrency("NZD", "New Zealand dollar", "$", 2),
    CatalogCurrency("OMR", "Omani rial", "ر.ع.", 3),
    CatalogCurrency("PAB", "Panamanian balboa", "B/.", 2),
    CatalogCurrency("PEN", "Peruvian sol", "S/", 2),
    CatalogCurrency("PGK", "Papua New Guinean kina", "K", 2),
    CatalogCurrency("PHP", "Philippine peso", "₱", 2),
    CatalogCurrency("PKR", "Pakistani rupee", "Rs", 2),
    CatalogCurrency("PLN", "Polish złoty", "zł", 2),
    CatalogCurrency("PYG", "Paraguayan guaraní", "₲", 0),
    CatalogCurrency("QAR", "Qatari riyal", "ر.ق", 2),
    CatalogCurrency("RON", "Romanian leu", "lei", 2),
    CatalogCurrency("RSD", "Serbian dinar", "дин.", 2),
    CatalogCurrency("RUB", "Russian ruble", "₽", 2),
    CatalogCurrency("RWF", "Rwandan franc", "FRw", 0),
    CatalogCurrency("SAR", "Saudi riyal", "ر.س", 2),
    CatalogCurrency("SBD", "Solomon Islands dollar", "$", 2),
    CatalogCurrency("SCR", "Seychelles rupee", "Rs", 2),
    CatalogCurrency("SDG", "Sudanese pound", "ج.س.", 2),
    CatalogCurrency("SEK", "Swedish krona", "kr", 2),
    CatalogCurrency("SGD", "Singapore dollar", "$", 2),
    CatalogCurrency("SHP", "Saint Helena pound", "£", 2),
    CatalogCurrency("SLE", "Sierra Leonean leone", "Le", 2),
    CatalogCurrency("SOS", "Somali shilling", "Sh", 2),
    CatalogCurrency("SRD", "Surinamese dollar", "$", 2),
    CatalogCurrency("SSP", "South Sudanese pound", "£", 2),
    CatalogCurrency("STN", "São Tomé and Príncipe dobra", "Db", 2),
    CatalogCurrency("SVC", "Salvadoran colón", "₡", 2),
    CatalogCurrency("SYP", "Syrian pound", "£S", 2),
    CatalogCurrency("SZL", "Swazi lilangeni", "E", 2),
    CatalogCurrency("THB", "Thai baht", "฿", 2),
    CatalogCurrency("TJS", "Tajikistani somoni", "SM", 2),
    CatalogCurrency("TMT", "Turkmenistan manat", "m", 2),
    CatalogCurrency("TND", "Tunisian dinar", "د.ت", 3),
    CatalogCurrency("TOP", "Tongan paʻanga", "T$", 2),  # noqa: RUF001
    CatalogCurrency("TRY", "Turkish lira", "₺", 2),
    CatalogCurrency("TTD", "Trinidad and Tobago dollar", "$", 2),
    CatalogCurrency("TWD", "New Taiwan dollar", "$", 2),
    CatalogCurrency("TZS", "Tanzanian shilling", "TSh", 2),
    CatalogCurrency("UAH", "Ukrainian hryvnia", "₴", 2),
    CatalogCurrency("UGX", "Ugandan shilling", "USh", 0),
    CatalogCurrency("USD", "US dollar", "$", 2),
    CatalogCurrency("UYU", "Uruguayan peso", "$", 2),
    CatalogCurrency("UZS", "Uzbekistani som", "soʻm", 2),  # noqa: RUF001
    CatalogCurrency("VES", "Venezuelan bolívar", "Bs.", 2),
    CatalogCurrency("VND", "Vietnamese đồng", "₫", 0),
    CatalogCurrency("VUV", "Vanuatu vatu", "VT", 0),
    CatalogCurrency("WST", "Samoan tālā", "T", 2),
    CatalogCurrency("XAF", "Central African CFA franc", "FCFA", 0),
    CatalogCurrency("XCD", "East Caribbean dollar", "$", 2),
    CatalogCurrency("XCG", "Caribbean guilder", "Cg", 2),
    CatalogCurrency("XOF", "West African CFA franc", "CFA", 0),
    CatalogCurrency("XPF", "CFP franc", "F", 0),
    CatalogCurrency("YER", "Yemeni rial", "﷼", 2),
    CatalogCurrency("ZAR", "South African rand", "R", 2),
    CatalogCurrency("ZMW", "Zambian kwacha", "ZK", 2),
    CatalogCurrency("ZWG", "Zimbabwe gold", "ZiG", 2),
)


def catalog_currency(code: str) -> CatalogCurrency | None:
    """The catalog's currency with code ``code``, if it has it."""
    return next((c for c in CATALOG if c.code == code), None)


def shown_name(name: str) -> str:
    """A register's currency name in the reader's language: a catalog name
    is translated; a name the user wrote is shown as written."""
    return tr(name)


# How a drawing writes amounts (maintainer, 2026-10-27, as accounting
# standards allow): its thousands and decimal separators.

#: The thousands separators offered: comma, point, space.
THOUSANDS = (",", ".", " ")
#: The decimal separators offered: point, comma.
DECIMAL = (".", ",")
#: Spaces a typed amount may group thousands with when the format's is a space.
SPACES = (" ", "\u00a0", "\u202f")
#: Digits, the ASCII ones only.
DIGITS = re.compile(r"[0-9]+")


class CurrencyRegister(NamedTuple):
    """A drawing's currencies, its principal one's code and how it writes
    amounts, as the Currencies dialog proposes them."""

    currencies: list[CadCurrency]
    principal: str
    thousands: str
    decimal: str


def format_amount(amount: str, thousands: str, decimal: str) -> str:
    """``amount`` as the core writes it (``"1250.50"``) with the drawing's
    separators: ``"1,250.50"``, ``"1.250,50"``, ``"1 250,50"``…"""
    whole, _, fraction = amount.partition(".")
    groups: list[str] = []
    while len(whole) > 3:
        groups.insert(0, whole[-3:])
        whole = whole[:-3]
    groups.insert(0, whole)
    text = thousands.join(groups)
    return f"{text}{decimal}{fraction}" if fraction else text


def parse_amount(typed: str, thousands: str, decimal: str) -> str:
    """An amount typed with the drawing's separators, as the core reads it
    (``"1.250,5"`` → ``"1250.5"``). Thousands are optional but, when
    written, in groups of three. Raises ValueError, its message for the
    user, when it is not such an amount; the core checks the decimals."""
    text = typed.strip()
    if thousands == " ":
        for space in SPACES:
            text = text.replace(space, " ")
    whole, point, fraction = text.partition(decimal)
    groups = whole.split(thousands)
    grouped = len(groups) == 1 or (
        1 <= len(groups[0]) <= 3 and all(len(g) == 3 for g in groups[1:])
    )
    whole = "".join(groups)
    if not grouped or not DIGITS.fullmatch(whole) or (point and not DIGITS.fullmatch(fraction)):
        example = format_amount("1250.50", thousands, decimal)
        raise ValueError(
            tr(
                "{typed} is not an amount: write it as {example}.",
                typed=typed.strip(),
                example=example,
            )
        )
    return f"{whole}.{fraction}" if point else whole
