"""`arqstudiocad --check`: verify that an installation or bundle is complete.

Every package build runs it; a bundle that fails is not published (ADR 0009).
"""

from collections.abc import Callable
from dataclasses import dataclass

from arqstudiocad.i18n import SOURCE_LANGUAGE, LanguagePackError, discover


@dataclass(frozen=True)
class CheckResult:
    name: str
    ok: bool
    detail: str


def check_core() -> CheckResult:
    try:
        from arqstudiocad import _core
    except ImportError as exc:
        return CheckResult("native core", False, str(exc))
    return CheckResult("native core", True, f"version {_core.__version__}")


def check_languages() -> CheckResult:
    try:
        packs = discover(strict=True)
    except LanguagePackError as exc:
        return CheckResult("language packs", False, str(exc))
    missing = {SOURCE_LANGUAGE, "es"} - packs.keys()
    if missing:
        return CheckResult("language packs", False, f"missing: {', '.join(sorted(missing))}")
    return CheckResult("language packs", True, ", ".join(sorted(packs)))


def check_credits() -> CheckResult:
    from arqstudiocad.credits import load_credits

    credits = load_credits()
    if credits is None:
        return CheckResult("credits (AUTHORS)", False, "AUTHORS file not found")
    if not credits.maintainers:
        return CheckResult("credits (AUTHORS)", False, "no maintainer listed")
    return CheckResult(
        "credits (AUTHORS)",
        True,
        f"{len(credits.maintainers)} maintainer(s), {len(credits.contributors)} contributor(s)",
    )


def check_qt() -> CheckResult:
    try:
        from PySide6.QtCore import qVersion
        from PySide6.QtWidgets import QApplication

        app = QApplication.instance() or QApplication([])
        platform_name = app.platformName() if isinstance(app, QApplication) else "?"
    except Exception as exc:  # any failure here means a broken Qt installation
        return CheckResult("Qt", False, f"{type(exc).__name__}: {exc}")
    return CheckResult("Qt", True, f"Qt {qVersion()}, platform plugin '{platform_name}'")


def check_settings() -> CheckResult:
    from arqstudiocad.paths import portable_dir

    folder = portable_dir()
    if folder is None:
        return CheckResult("settings", True, "system location")
    probe = folder / ".write-test"
    try:
        probe.write_text("", encoding="utf-8")
        probe.unlink()
    except OSError as exc:
        return CheckResult("settings", False, f"portable folder is not writable: {exc}")
    return CheckResult("settings", True, f"portable, in {folder}")


CHECKS: tuple[Callable[[], CheckResult], ...] = (
    check_core,
    check_languages,
    check_credits,
    check_qt,
    check_settings,
)


def run_checks() -> list[CheckResult]:
    return [check() for check in CHECKS]


def main() -> int:
    results = run_checks()
    for result in results:
        print(f"[{'OK' if result.ok else 'FAIL':>4}] {result.name}: {result.detail}")
    return 0 if all(result.ok for result in results) else 1
