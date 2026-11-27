"""The CAD core's Python API (ADR 0012).

Application code imports from here, never from the native module
``arqstudiocad._core``. Angles are radians; degrees exist only in the
interface. Points are returned as ``(x, y)`` tuples and accepted as any two
numbers (tuple, list, NumPy array).
"""

from typing import Protocol, SupportsFloat

from arqstudiocad._core import (
    CadApplication,
    CadArc,
    CadBlockData,
    CadBlockInfo,
    CadBounds2,
    CadChangeSet,
    CadCircle,
    CadColor,
    CadCurrency,
    CadDocument,
    CadEllipse,
    CadEntityInfo,
    CadError,
    CadGrip,
    CadGroup,
    CadIoError,
    CadLayerInfo,
    CadLine,
    CadMassProperties,
    CadRectangle,
    CadSnap,
    CadSolidMeasures,
    CadSpline,
    CadText,
    CadView,
    DamagedFileError,
    DocumentNotFoundError,
    EntityNotFoundError,
    InvalidGeometryError,
    InvalidOperationError,
    LayerNotFoundError,
    LossyConversionError,
    OverwritesDamagedError,
    SaveNotVerifiedError,
    UnsupportedFormatError,
    __version__,
    set_writer,
)
from arqstudiocad.product import APP_NAME

# Every DWG saved names this program in its AppInfo section, so a drawing
# saved here is known as ours when read again (ADR 0045 D1).
set_writer(APP_NAME, __version__)

type Point = tuple[float, float]
"""A point or vector as the core returns it."""


class PointLike(Protocol):
    """A point or vector as the core accepts it: any two numbers, such as a
    tuple, a list or a NumPy array of shape (2,)."""

    def __len__(self) -> int: ...
    def __getitem__(self, index: int, /) -> SupportsFloat: ...


__all__ = [
    "CadApplication",
    "CadArc",
    "CadBlockData",
    "CadBlockInfo",
    "CadBounds2",
    "CadChangeSet",
    "CadCircle",
    "CadColor",
    "CadCurrency",
    "CadDocument",
    "CadEllipse",
    "CadEntityInfo",
    "CadError",
    "CadGrip",
    "CadGroup",
    "CadIoError",
    "CadLayerInfo",
    "CadLine",
    "CadMassProperties",
    "CadRectangle",
    "CadSnap",
    "CadSolidMeasures",
    "CadSpline",
    "CadText",
    "CadView",
    "DamagedFileError",
    "DocumentNotFoundError",
    "EntityNotFoundError",
    "InvalidGeometryError",
    "InvalidOperationError",
    "LayerNotFoundError",
    "LossyConversionError",
    "OverwritesDamagedError",
    "Point",
    "PointLike",
    "SaveNotVerifiedError",
    "UnsupportedFormatError",
]
