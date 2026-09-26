# Ruby binding

The gem calls the shared C ABI through Ruby's standard-library `Fiddle`.
Build/install the native library as described in `../../README.md` before use.

```ruby
require "distill_strip_ansi"

DistillStripAnsi.strip("red: \e[31mtext\e[0m") # => "red: text"
DistillStripAnsi.contains_ansi?("\e[31mtext") # => true
```

Set `DISTILL_STRIP_ANSI_LIB` to the shared library path when it is not
installed in the operating system's normal library search path.