"""Credits read from the AUTHORS file, the single source shown in Help > About."""

from dataclasses import dataclass

from arqstudiocad.paths import authors_file


@dataclass(frozen=True)
class Credits:
    maintainers: tuple[str, ...]
    contributors: tuple[str, ...]


def parse_authors(text: str) -> Credits:
    """Parse AUTHORS: a heading line starting with "Maintainer" or "Contributor"
    opens a section, and each indented line under it is one person."""
    sections: dict[str, list[str]] = {"maintainers": [], "contributors": []}
    current: list[str] | None = None
    for line in text.splitlines():
        if not line.strip():
            continue
        if not line[0].isspace():
            heading = line.casefold()
            if heading.startswith("maintainer"):
                current = sections["maintainers"]
            elif heading.startswith("contributor"):
                current = sections["contributors"]
            else:
                current = None
        elif current is not None:
            current.append(line.strip())
    return Credits(tuple(sections["maintainers"]), tuple(sections["contributors"]))


def load_credits() -> Credits | None:
    """Credits from the installed AUTHORS file, or None if it is missing."""
    path = authors_file()
    if path is None:
        return None
    return parse_authors(path.read_text(encoding="utf-8"))
