from arqstudiocad.credits import load_credits, parse_authors

SAMPLE = """\
ArqStudioCAD is written by its contributors.

Maintainer:

  @someone (GitHub)

Contributors (add yourself):

  Ana Pérez — translations
  Luis <luis@example.org>
"""


def test_parse_authors_sections() -> None:
    credits = parse_authors(SAMPLE)
    assert credits.maintainers == ("@someone (GitHub)",)
    assert credits.contributors == ("Ana Pérez — translations", "Luis <luis@example.org>")


def test_repository_authors_lists_a_maintainer() -> None:
    credits = load_credits()
    assert credits is not None
    assert credits.maintainers
