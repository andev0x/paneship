# macOS cross-link stubs

These stubs are used only by the release workflow when cross-compiling the
`x86_64-apple-darwin` and `aarch64-apple-darwin` binaries on `ubuntu-latest`
with `cargo-zigbuild`.

Zig ships only `libSystem.tbd` for Darwin targets, but the Rust `libc` crate
unconditionally passes `-liconv` on Apple targets. `libiconv.tbd` is a minimal
TAPI stub that declares the real system install name so the linker can produce a
normal Mach-O load command. No Apple SDK is downloaded or required.

When a dependency starts referencing iconv functions, add the missing symbols to
`libiconv.tbd`. Keep the file free of YAML comments: Zig's TAPI parser rejects
comment lines in this file.
