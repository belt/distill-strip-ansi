# Language bindings

The Rust crate is the canonical API. C, Python, Go, Ruby, and TypeScript share
the versioned C ABI in `c-abi`, so parsing behavior and malformed-sequence
handling remain consistent across languages.

Build the shared library from the repository root:

```sh
cargo build --release -p distill-strip-ansi-c
```

Set `DISTILL_STRIP_ANSI_LIB` to the resulting library path when it is not in
the platform's normal library search path. The default build output is
`target/release/libdistill_strip_ansi_c.so` on Linux,
`target/release/libdistill_strip_ansi_c.dylib` on macOS, and
`target/release/distill_strip_ansi_c.dll` on Windows.

| Language | Package/API | Runtime bridge |
| --- | --- | --- |
| Rust | `distill-strip-ansi` crate (`strip`, `strip_str`, `StripStream`) | Native Rust API |
| C | Header in `c-abi/include/distill_strip_ansi.h` | C ABI shared/static library |
| Python | `python/`, `strip_bytes`, `strip_text`, `contains_ansi` | Standard-library `ctypes` |
| Go | `go/`, `Strip`, `StripString`, `ContainsANSI` | cgo; link the C ABI library |
| Ruby | `ruby/`, `DistillStripAnsi.strip`, `contains_ansi?` | Standard-library `Fiddle` |
| TypeScript | `typescript/`, `strip`, `containsAnsi` | Koffi; install npm package dependencies |

## Smoke tests

After building the library, Python tests can be run with:

```sh
PYTHONPATH=bindings/python/src python3 -m unittest discover -s bindings/python/tests
```

Go requires the C library on the linker and runtime search paths:

```sh
CGO_LDFLAGS="-L$PWD/target/release" LD_LIBRARY_PATH="$PWD/target/release" go test ./bindings/go/...
```

Ruby tests use only the standard library:

```sh
ruby bindings/ruby/test/test_distill_strip_ansi.rb
```

TypeScript:

```sh
cd bindings/typescript
npm install
npm run build
npm test
```