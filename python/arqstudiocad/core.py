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
    CadBounds2,
    CadChangeSet,
    CadCircle,
    CadColor,
    CadDocument,
    CadEntityInfo,
    CadError,
    CadGroup,
    CadIoError,
    CadLayerInfo,
    CadLine,
    DocumentNotFoundError,
    EntityNotFoundError,
    InvalidGeometryError,
    InvalidOperationError,
    LayerNotFoundError,
    UnsupportedFormatError,
)

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
    "CadBounds2",
    "CadChangeSet",
    "CadCircle",
    "CadColor",
    "CadDocument",
    "CadEntityInfo",
    "CadError",
    "CadGroup",
    "CadIoError",
    "CadLayerInfo",
    "CadLine",
    "DocumentNotFoundError",
    "EntityNotFoundError",
    "InvalidGeometryError",
    "InvalidOperationError",
    "LayerNotFoundError",
    "Point",
    "PointLike",
    "UnsupportedFormatError",
]
