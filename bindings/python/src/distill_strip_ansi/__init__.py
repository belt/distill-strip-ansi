"""Python bindings for the distill-strip-ansi C ABI."""

from __future__ import annotations

import ctypes
import ctypes.util
import os
import sys
from pathlib import Path
from typing import TypeAlias

__all__ = ["contains_ansi", "strip_bytes", "strip_text"]

BytesLike: TypeAlias = bytes | bytearray | memoryview
_LIBRARY: ctypes.CDLL | None = None
_ABI_VERSION = 1


def _library_candidates() -> list[str]:
    configured = os.environ.get("DISTILL_STRIP_ANSI_LIB")
    candidates = [configured] if configured else []
    if sys.platform == "darwin":
        names = ("libdistill_strip_ansi_c.dylib",)
    elif os.name == "nt":
        names = ("distill_strip_ansi_c.dll",)
    else:
        names = ("libdistill_strip_ansi_c.so",)
    project_root = Path(__file__).resolve().parents[4]
    for profile in ("release", "debug"):
        candidates.extend(str(project_root / "target" / profile / name) for name in names)
    discovered = ctypes.util.find_library("distill_strip_ansi_c")
    if discovered:
        candidates.append(discovered)
    candidates.extend(names)
    return candidates


def _load_library() -> ctypes.CDLL:
    global _LIBRARY
    if _LIBRARY is not None:
        return _LIBRARY
    errors: list[str] = []
    for candidate in _library_candidates():
        try:
            library = ctypes.CDLL(candidate)
            library.dsa_abi_version.argtypes = []
            library.dsa_abi_version.restype = ctypes.c_uint32
            library.dsa_strip.argtypes = [
                ctypes.POINTER(ctypes.c_uint8),
                ctypes.c_size_t,
                ctypes.POINTER(ctypes.c_uint8),
            ]
            library.dsa_strip.restype = ctypes.c_ssize_t
            library.dsa_contains_ansi.argtypes = [ctypes.POINTER(ctypes.c_uint8), ctypes.c_size_t]
            library.dsa_contains_ansi.restype = ctypes.c_int32
            version = library.dsa_abi_version()
            if version != _ABI_VERSION:
                raise RuntimeError(f"unsupported distill-strip-ansi C ABI version {version}")
            _LIBRARY = library
            return library
        except (OSError, AttributeError) as error:
            errors.append(f"{candidate}: {error}")
    raise OSError(
        "Could not load libdistill_strip_ansi_c; build it and set "
        "DISTILL_STRIP_ANSI_LIB. Tried: " + "; ".join(errors)
    )


def _buffer(data: bytes) -> tuple[ctypes.Array[ctypes.c_char] | None, ctypes.POINTER(ctypes.c_uint8)]:
    if not data:
        return None, ctypes.POINTER(ctypes.c_uint8)()
    storage = ctypes.create_string_buffer(data, len(data))
    return storage, ctypes.cast(storage, ctypes.POINTER(ctypes.c_uint8))


def strip_bytes(data: BytesLike) -> bytes:
    """Return bytes with ANSI control sequences removed."""
    raw = bytes(data)
    library = _load_library()
    input_storage, input_pointer = _buffer(raw)
    output_storage, output_pointer = _buffer(bytes(len(raw)))
    output_len = library.dsa_strip(input_pointer, len(raw), output_pointer)
    if output_len == -1:
        raise ValueError("invalid argument passed to distill-strip-ansi")
    if output_len < 0:
        raise RuntimeError(f"distill-strip-ansi failed with status {output_len}")
    return output_storage.raw[:output_len] if output_storage is not None else b""


def strip_text(text: str) -> str:
    """Strip ANSI sequences from text, preserving its UTF-8 encoding."""
    return strip_bytes(text.encode("utf-8")).decode("utf-8")


def contains_ansi(data: BytesLike) -> bool:
    """Return whether the bytes contain an ANSI escape sequence."""
    raw = bytes(data)
    library = _load_library()
    input_storage, input_pointer = _buffer(raw)
    result = library.dsa_contains_ansi(input_pointer, len(raw))
    if result < 0:
        raise ValueError("invalid argument passed to distill-strip-ansi")
    return bool(result)