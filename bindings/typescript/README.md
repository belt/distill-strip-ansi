# TypeScript binding

The package calls the shared C ABI through [Koffi](https://koffi.dev/).
Build/install the native library as described in `../../README.md`, then
install the package dependencies with npm.

```ts
import { containsAnsi, strip } from "distill-strip-ansi";

strip(Buffer.from("red: \x1b[31mtext\x1b[0m")); // Buffer containing "red: text"
containsAnsi(Buffer.from("\x1b[31mtext")); // true
```

Set `DISTILL_STRIP_ANSI_LIB` to the shared library path when it is not
installed in the operating system's normal library search path.