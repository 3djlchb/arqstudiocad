"""The program a drawing comes from (ADR 0045 D1): a DWG saved here names
us, a DXF says nothing, AutoCAD's public samples name AutoCAD."""

from pathlib import Path

import pytest

from arqstudiocad.core import CadApplication
from arqstudiocad.product import APP_NAME

CORPUS = Path(__file__).resolve().parents[2] / "target" / "corpus" / "libredwg"


def test_a_drawing_made_here_and_saved_as_dwg_is_ours(tmp_path: Path) -> None:
    app = CadApplication()
    doc = app.create_document("made")
    assert doc.producer == ("ours", "")
    doc.add_line((0.0, 0.0), (1.0, 0.0))
    path = tmp_path / "made.dwg"
    doc.save(path)
    again = CadApplication().open_document(path)
    assert again.producer == ("ours", APP_NAME)


def test_a_dxf_names_no_producer(tmp_path: Path) -> None:
    doc = CadApplication().create_document("made")
    path = tmp_path / "made.dxf"
    doc.save(path)
    assert CadApplication().open_document(path).producer == ("unknown", "")


@pytest.mark.skipif(not CORPUS.is_dir(), reason="needs tools/fetch_codec_samples.py")
def test_autocad_s_drawings_name_autocad_and_older_ones_nobody() -> None:
    app = CadApplication()
    assert app.open_document(CORPUS / "example_2007.dwg").producer == ("autocad", "AutoCAD")
    assert app.open_document(CORPUS / "example_2000.dwg").producer == ("unknown", "")
