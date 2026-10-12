"""CadApplication and CadDocument from Python (ADR 0012, step 4.2)."""

import threading
import time
from collections.abc import Callable
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

import pytest

from arqstudiocad.core import (
    CadApplication,
    CadDocument,
    CadIoError,
    DocumentNotFoundError,
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
    assert app.document_ids() == [doc.id]
    assert repr(doc) == f"CadDocument(id={doc.id}, name='Plano1')"
    assert repr(app.create_document("it's")) == f'CadDocument(id={doc.id + 1}, name="it\'s")'


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


def _big_dxf(path: Path, lines: int) -> Path:
    """An R12 DXF with many LINE entities: slow enough to open and save."""
    codes = ["0", "SECTION", "2", "HEADER", "9", "$ACADVER", "1", "AC1009", "0", "ENDSEC"]
    codes += ["0", "SECTION", "2", "ENTITIES"]
    for i in range(lines):
        codes += ["0", "LINE", "8", "0", "10", f"{i}.0", "20", "0.0", "30", "0.0"]
        codes += ["11", f"{i}.5", "21", "1.0", "31", "0.0"]
    codes += ["0", "ENDSEC", "0", "EOF", ""]
    path.write_text("\n".join(codes), encoding="ascii")
    return path


def _share_of_time_other_threads_ran(call: Callable[[], object]) -> float:
    """How much a counting thread advanced during `call`, relative to its pace
    when the main thread just sleeps: near 1 if `call` released the GIL, near 0
    if it held it (then only a switch interval or two slips through)."""
    ticks = 0
    done = threading.Event()

    def count() -> None:
        nonlocal ticks
        while not done.is_set():
            ticks += 1

    counter = threading.Thread(target=count)
    counter.start()
    try:
        start, t0 = ticks, time.perf_counter()
        time.sleep(0.2)
        pace = (ticks - start) / (time.perf_counter() - t0)
        start, t0 = ticks, time.perf_counter()
        call()
        elapsed = time.perf_counter() - t0
        ran = ticks - start
    finally:
        done.set()
        counter.join()
    assert elapsed > 0.1, f"the call took only {elapsed:.3f} s: too short to tell"
    return ran / (pace * elapsed)


def test_opening_and_saving_release_the_gil(app: CadApplication, tmp_path: Path) -> None:
    source = _big_dxf(tmp_path / "grande.dxf", 200_000)
    opened: list[CadDocument] = []
    share = _share_of_time_other_threads_ran(lambda: opened.append(app.open_document(source)))
    assert share > 0.3, f"other threads ran {share:.0%} of the time while opening"
    doc = opened[0]
    share = _share_of_time_other_threads_ran(lambda: doc.save(tmp_path / "copia.dwg"))
    assert share > 0.3, f"other threads ran {share:.0%} of the time while saving"
