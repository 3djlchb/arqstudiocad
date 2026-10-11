"""Translation of user-facing text (ADR 0006).

Everything the user reads goes through `tr()`; everything the user types
(command names, prompt option keys) stays English. A language is a folder
under `arqstudiocad/locales/<code>/` with `meta.json` and `ui.json`; adding
one needs no code change. A missing translation falls back to the English
source, never to a key or an error.
"""

import json
import logging
from dataclasses import dataclass, field
from pathlib import Path

from arqstudiocad.paths import locales_dir
from arqstudiocad.product import APP_NAME

__all__ = [
    "SOURCE_LANGUAGE",
    "LanguagePack",
    "LanguagePackError",
    "available_languages",
    "current_language",
    "discover",
    "load_pack",
    "set_language",
    "tr",
]

SOURCE_LANGUAGE = "en"

_log = logging.getLogger(__name__)


class LanguagePackError(ValueError):
    """A language folder that cannot be loaded."""


@dataclass(frozen=True)
class LanguagePack:
    """One installed language: its metadata and its `{english: translation}` catalog."""

    code: str
    name: str
    maintainer: str = ""
    maintained: bool = False
    ui: dict[str, str] = field(default_factory=dict)


def _read_json_object(path: Path) -> dict[str, object]:
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise LanguagePackError(f"{path}: {exc}") from exc
    if not isinstance(data, dict):
        raise LanguagePackError(f"{path}: expected a JSON object")
    return data


def load_pack(folder: Path) -> LanguagePack:
    """Load and validate the language pack in `folder`."""
    meta = _read_json_object(folder / "meta.json")
    code, name = meta.get("code"), meta.get("name")
    if code != folder.name:
        raise LanguagePackError(f"{folder}: meta.json code {code!r} must match the folder name")
    if not isinstance(name, str) or not name:
        raise LanguagePackError(f"{folder}: meta.json needs a non-empty 'name'")
    maintainer = meta.get("maintainer", "")
    maintained = meta.get("maintained", False)

    ui: dict[str, str] = {}
    ui_path = folder / "ui.json"
    if ui_path.exists():
        for source, translation in _read_json_object(ui_path).items():
            if not isinstance(translation, str):
                raise LanguagePackError(f"{ui_path}: translation of {source!r} is not a string")
            ui[source] = translation

    return LanguagePack(
        code=folder.name,
        name=name,
        maintainer=maintainer if isinstance(maintainer, str) else "",
        maintained=maintained is True,
        ui=ui,
    )


def discover(root: Path | None = None, *, strict: bool = False) -> dict[str, LanguagePack]:
    """Load every language folder under `root`.

    A broken pack is skipped with a warning so the application still starts;
    with `strict=True` (used by `--check` and the tests) it raises instead.
    """
    root = root if root is not None else locales_dir()
    packs: dict[str, LanguagePack] = {}
    for folder in sorted(p for p in root.iterdir() if p.is_dir()):
        try:
            pack = load_pack(folder)
        except LanguagePackError:
            if strict:
                raise
            _log.warning("Skipping invalid language pack %s", folder, exc_info=True)
            continue
        packs[pack.code] = pack
    return packs


_packs: dict[str, LanguagePack] | None = None
_active: LanguagePack | None = None


def _installed() -> dict[str, LanguagePack]:
    global _packs
    if _packs is None:
        _packs = discover()
    return _packs


def available_languages() -> list[LanguagePack]:
    """Installed languages, sorted by their own name."""
    return sorted(_installed().values(), key=lambda pack: pack.name.casefold())


def set_language(code: str) -> bool:
    """Make `code` the active language. Returns False if it is not installed."""
    global _active
    pack = _installed().get(code)
    if pack is None:
        return False
    _active = pack
    return True


def current_language() -> str:
    """Code of the active language."""
    return _active.code if _active is not None else SOURCE_LANGUAGE


def tr(source: str, /, **values: object) -> str:
    """Translate `source` into the active language, then fill `{placeholders}`.

    `{app}` is always available and expands to the product name, so no
    translatable string spells the name out.
    """
    text = _active.ui.get(source, source) if _active is not None else source
    if not values and "{app}" not in text:
        return text
    values = {"app": APP_NAME, **values}
    try:
        return text.format(**values)
    except (KeyError, IndexError, ValueError):
        # A broken translation must never hide data from the user.
        _log.warning("Bad placeholders in translation of %r", source)
        return source.format(**values)
