# Python binding

This package uses Python's standard-library `ctypes` to call the shared C ABI.
Build/install the native library as described in `../../README.md` before use.

```python
from distill_strip_ansi import contains_ansi, strip_bytes, strip_text

assert strip_bytes(b"red: \x1b[31mtext\x1b[0m") == b"red: text"
assert strip_text("red: \x1b[31mtext\x1b[0m") == "red: text"
assert contains_ansi(b"\x1b[31mtext")
```

Set `DISTILL_STRIP_ANSI_LIB` to the shared library path when it is not
installed in the operating system's normal library search path.