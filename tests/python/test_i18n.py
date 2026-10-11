import ast
import json
import string
from pathlib import Path

import pytest

from arqstudiocad.i18n import (
    SOURCE_LANGUAGE,
    LanguagePackError,
    current_language,
    discover,
    load_pack,
    set_language,
    tr,
)
from arqstudiocad.paths import package_dir
from arqstudiocad.product import APP_NAME

SOURCES = sorted(package_dir().rglob("*.py"))
PACKS = discover(strict=True)


def _tr_literals() -> set[str]:
    """Every string literal passed as the first argument of a tr() call."""
    found: set[str] = set()
    for path in SOURCES:
        for node in ast.walk(ast.parse(path.read_text(encoding="utf-8"))):
            if (
                isinstance(node, ast.Call)
                and isinstance(node.func, ast.Name)
                and node.func.id == "tr"
                and node.args
                and isinstance(node.args[0], ast.Constant)
                and isinstance(node.args[0].value, str)
            ):
                found.add(node.args[0].value)
    return found


def _all_string_literals() -> set[str]:
    found: set[str] = set()
    for path in SOURCES:
        for node in ast.walk(ast.parse(path.read_text(encoding="utf-8"))):
            if isinstance(node, ast.Constant) and isinstance(node.value, str):
                found.add(node.value)
    return found


def _fields(text: str) -> set[str]:
    return {name for _, name, _, _ in string.Formatter().parse(text) if name}


def test_source_and_spanish_packs_are_installed() -> None:
    assert {SOURCE_LANGUAGE, "es"} <= PACKS.keys()
    assert PACKS[SOURCE_LANGUAGE].ui == {}, "English is the source; its catalog stays empty"


def test_scanner_finds_strings() -> None:
    # A check that inspects nothing is a failure, not a pass.
    assert len(_tr_literals()) >= 10


@pytest.mark.parametrize("code", [c for c, p in PACKS.items() if p.maintained and c != "en"])
def test_maintained_packs_translate_every_string(code: str) -> None:
    missing = sorted(_tr_literals() - PACKS[code].ui.keys())
    assert not missing, f"{code}: untranslated strings: {missing}"


@pytest.mark.parametrize("code", sorted(PACKS))
def test_no_dead_keys(code: str) -> None:
    # Keys may reach tr() through a variable, so any string literal counts.
    dead = sorted(PACKS[code].ui.keys() - _all_string_literals())
    assert not dead, f"{code}: keys not found in the sources: {dead}"


@pytest.mark.parametrize("code", sorted(PACKS))
def test_translations_keep_placeholders(code: str) -> None:
    broken = {
        source: translation
        for source, translation in PACKS[code].ui.items()
        if _fields(source) != _fields(translation)
    }
    assert not broken, f"{code}: placeholders changed: {broken}"


def test_tr_translates_and_falls_back_to_english() -> None:
    assert tr("&File") == "&File"
    assert set_language("es")
    assert current_language() == "es"
    assert tr("&File") == "&Archivo"
    assert tr("A string nobody translated") == "A string nobody translated"


def test_tr_fills_placeholders_in_translation() -> None:
    set_language("es")
    text = tr("{app} {version} — CAD core {core}", version="1.2", core="3.4")
    assert text == f"{APP_NAME} 1.2 — núcleo CAD 3.4"


def test_no_translatable_string_spells_the_product_name() -> None:
    # The name lives in arqstudiocad.product only; strings use {app}.
    spelled = sorted(s for s in _tr_literals() if APP_NAME.casefold() in s.casefold())
    assert not spelled, f"use {{app}} instead of the product name: {spelled}"
    for code, pack in PACKS.items():
        spelled = sorted(t for t in pack.ui.values() if APP_NAME.casefold() in t.casefold())
        assert not spelled, f"{code}: use {{app}} instead of the product name: {spelled}"


def test_tr_fills_the_product_name() -> None:
    assert tr("About {app}") == f"About {APP_NAME}"
    set_language("es")
    assert tr("About {app}") == f"Acerca de {APP_NAME}"


def test_unknown_language_is_refused() -> None:
    assert not set_language("xx")
    assert current_language() == SOURCE_LANGUAGE


def _write_pack(root: Path, code: str, meta: object, ui: object | None = None) -> Path:
    folder = root / code
    folder.mkdir()
    (folder / "meta.json").write_text(json.dumps(meta), encoding="utf-8")
    if ui is not None:
        (folder / "ui.json").write_text(json.dumps(ui), encoding="utf-8")
    return folder


def test_dropping_a_folder_adds_a_language(tmp_path: Path) -> None:
    _write_pack(tmp_path, "qu", {"code": "qu", "name": "Runa Simi"}, {"&File": "&Willay"})
    packs = discover(tmp_path, strict=True)
    assert packs["qu"].name == "Runa Simi"
    assert packs["qu"].ui["&File"] == "&Willay"
    assert not packs["qu"].maintained


def test_pack_code_must_match_folder(tmp_path: Path) -> None:
    folder = _write_pack(tmp_path, "fr", {"code": "es", "name": "Français"})
    with pytest.raises(LanguagePackError):
        load_pack(folder)


def test_broken_pack_is_skipped_unless_strict(tmp_path: Path) -> None:
    _write_pack(tmp_path, "de", {"code": "de", "name": "Deutsch"})
    broken = tmp_path / "zz"
    broken.mkdir()
    (broken / "meta.json").write_text("{not json", encoding="utf-8")
    assert set(discover(tmp_path)) == {"de"}
    with pytest.raises(LanguagePackError):
        discover(tmp_path, strict=True)
