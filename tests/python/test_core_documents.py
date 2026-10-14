"""CadApplication and CadDocument from Python (ADR 0012, step 4.2)."""

from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

import pytest
from damaged_files import LINES, recovered_dxf, unreadable_dwg
from gil_helpers import big_dxf, share_of_time_other_threads_ran

from arqstudiocad.core import (
    CadApplication,
    CadDocument,
    CadIoError,
    DamagedFileError,
    DocumentNotFoundError,
    OverwritesDamagedError,
    SaveNotVerifiedError,
    UnsupportedFormatError,
)


@pytest.fixture
def app() -> CadApplication:
    return CadApplication()


def test_a_new_document(app: CadApplication) -> None:
    doc = app.create_document("Plano1")
    assert isinstance(doc, CadDocument)
    assert (doc.name, doc.path, doc.is_modified, doc.is_open) == ("Plano1", None, False, True)
    assert doc.version.startswith("AC")
    assert doc.entity_count == 0
    doc.add_line((0, 0), (1, 1))
    doc.add_circle((0, 0), 2)
    assert doc.entity_count == 2
    assert app.document_ids() == [doc.id]
    assert repr(doc) == f"CadDocument(id={doc.id}, name='Plano1')"
    assert repr(app.create_document("it's")) == f'CadDocument(id={doc.id + 1}, name="it\'s")'


def test_a_file_with_nothing_to_recover_is_refused(app: CadApplication, tmp_path: Path) -> None:
    with pytest.raises(DamagedFileError) as raised:
        app.open_document(unreadable_dwg(tmp_path / "cortado.dwg"))
    assert isinstance(raised.value, CadIoError), "old handlers still catch it"
    assert raised.value.code == "damaged_file"
    assert "damaged" in str(raised.value)
    assert app.documents() == []


def test_a_recovered_drawing_says_so_and_guards_its_file(
    app: CadApplication, tmp_path: Path
) -> None:
    assert app.create_document("x").recovery_problems == [], "nothing to report"
    path = recovered_dxf(tmp_path / "danado.dxf")
    original = path.read_bytes()
    doc = app.open_document(path)
    assert doc.entity_count == 5 < LINES
    assert any("zz" in p for p in doc.recovery_problems), doc.recovery_problems

    with pytest.raises(OverwritesDamagedError) as raised:
        doc.save(path)
    assert raised.value.code == "overwrites_damaged"
    assert path.read_bytes() == original
    doc.save(tmp_path / "copia.dxf")
    doc.allow_overwriting_damaged()
    doc.save(path)
    assert app.open_document(path).recovery_problems == []


def test_an_unverified_save_is_an_io_error_with_its_own_code() -> None:
    # Saving reads the file back (ADR 0017 D5.2); the core's tests make a
    # write fail, which Python cannot: here only the class is checked.
    assert issubclass(SaveNotVerifiedError, CadIoError)
    assert SaveNotVerifiedError.code == "save_not_verified"


def test_handles_to_the_same_document_are_equal(app: CadApplication) -> None:
    doc = app.create_document("Plano1")
    other = app.create_document("Plano2")
    assert app.document(doc.id) == doc
    assert app.documents() == [doc, other]
    assert len({doc, app.document(doc.id), other}) == 2
    assert doc != CadApplication().create_document("Plano1"), "another application"


@pytest.mark.parametrize("ext", ["dwg", "dxf"])
def test_save_then_open(app: CadApplication, tmp_path: Path, ext: str) -> None:
    doc = app.create_document("Plano1")
    path = tmp_path / f"vivienda.{ext}"
    doc.save(path)
    assert (doc.name, doc.path, doc.is_modified) == (f"vivienda.{ext}", path, False)
    assert path.is_file()

    again = app.open_document(str(path))  # str or Path
    assert again != doc
    assert (again.name, again.path, again.is_modified) == (f"vivienda.{ext}", path, False)
    assert app.document_ids() == [doc.id, again.id]

    copy = tmp_path / f"copia.{ext}"
    app.save_document(again.id, copy)
    assert again.path == copy


def test_closed_documents_raise_document_not_found(app: CadApplication) -> None:
    doc = app.create_document("Plano1")
    keep = app.create_document("Plano2")
    doc.close()
    assert not doc.is_open
    assert app.document_ids() == [keep.id]
    assert repr(doc) == f"CadDocument(id={doc.id}, closed)"
    for call in (
        lambda: doc.name,
        lambda: doc.path,
        lambda: doc.is_modified,
        lambda: doc.version,
        doc.close,
        lambda: app.close_document(doc.id),
        lambda: app.document(doc.id),
        lambda: doc.save("x.dxf"),
    ):
        with pytest.raises(DocumentNotFoundError) as caught:
            call()
        assert caught.value.document_id == doc.id
        assert caught.value.code == "document_not_found"
    assert app.create_document("Plano3").id not in (doc.id, keep.id), "ids are never reused"


def test_file_errors(app: CadApplication, tmp_path: Path) -> None:
    with pytest.raises(CadIoError):
        app.open_document(tmp_path / "no-existe.dwg")
    broken = tmp_path / "roto.dwg"
    broken.write_bytes(b"this is not a drawing")
    with pytest.raises(CadIoError):
        app.open_document(broken)
    with pytest.raises(UnsupportedFormatError):
        app.open_document(tmp_path / "plano.pdf")
    doc = app.create_document("Plano1")
    with pytest.raises(UnsupportedFormatError):
        doc.save(tmp_path / "plano.pdf")
    assert (doc.name, doc.path) == ("Plano1", None), "a failed save changes nothing"
    assert app.document_ids() == [doc.id], "failed opens add no document"


def test_documents_save_from_several_threads(app: CadApplication, tmp_path: Path) -> None:
    docs = [app.create_document(f"Plano{i}") for i in range(8)]

    def save(doc: CadDocument) -> Path:
        path = tmp_path / f"{doc.name}.dwg"
        doc.save(path)
        return path

    with ThreadPoolExecutor(max_workers=4) as pool:
        paths = list(pool.map(save, docs))
    assert all(p.is_file() for p in paths)
    assert [d.path for d in docs] == paths


def test_opening_and_saving_release_the_gil(app: CadApplication, tmp_path: Path) -> None:
    # Each call must last long enough to tell (0.1 s against a 5 ms switch
    # interval); on a fast machine the drawing grows until it does.
    for lines in (200_000, 400_000, 800_000):
        source = big_dxf(tmp_path / f"grande-{lines}.dxf", lines)
        opened: list[CadDocument] = []
        open_share, open_time = share_of_time_other_threads_ran(
            lambda: opened.append(app.open_document(source))  # noqa: B023
        )
        doc = opened[0]
        save_share, save_time = share_of_time_other_threads_ran(
            lambda: doc.save(tmp_path / f"copia-{lines}.dwg")  # noqa: B023
        )
        doc.close()
        if min(open_time, save_time) > 0.1:
            break
    else:
        pytest.fail(f"open {open_time:.3f} s, save {save_time:.3f} s: too short to tell")
    assert open_share > 0.3, f"other threads ran {open_share:.0%} of the time while opening"
    assert save_share > 0.3, f"other threads ran {save_share:.0%} of the time while saving"
