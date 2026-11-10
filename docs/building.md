# Building Specular

You need Rust from <https://rustup.rs>; the pinned nightly in `rust-toolchain.toml` installs itself
on first use. Everything is driven by `build.sh` (Git Bash or MSYS2 on Windows):

```sh
./build.sh [release|debug] [linux|windows|macos|host|all] [--package] [--pgo] [--sign] [--out DIR]
```

- Defaults are `release` for the host platform. Without `--package` it only compiles and copies
  the binary to `target/dist/<version>/<platform>/`.
- `--package` also builds the packages:
  - Linux: `.tar.gz`, `.AppImage` and `.flatpak`.
  - Windows: `.zip`, `.msi` (WiX v4, Windows only) and winget manifests.
  - macOS: `.app` inside a `.dmg` (Mac only).

  Debug packages get a `-debug` suffix.
- `--pgo` builds a profile-guided (plus BOLT, if available) Linux binary through
  `tools/release/pgo.sh`. It trains on the `tools/corpus` test images.
- `--sign` implies `--package`. Run `./build.sh --help` for the credentials it needs.
- `all` builds every platform the host can and ends with a summary table of what was built or
  skipped, and why.
- Windows cross-builds from Linux use `cargo-xwin`. They need you to accept Microsoft's MSVC/SDK
  licence (`SPECULAR_ACCEPT_MSVC_LICENSE=1`).
- Build output stays inside `target/`. `CARGO_TARGET_DIR` overrides the cargo directory, and
  downloaded tools go to `target/tools/`.

Besides Rust, building needs a C/C++ compiler (for vendored C and C++ such as libwebp and libjxl)
and cmake (for libjxl). Release builds also need `nasm`, for rav1e's hand-written assembly: `build.sh`, the
packaging scripts and `tools/release/pgo.sh` turn on the `specular/asm` feature, which makes AVIF
output in batch convert about 2.5× faster. Debug builds and a plain `cargo build` leave it off.

If required tools are missing, the script lists all of them, with how to install each, and stops.

Before running the tests in a fresh checkout, run `cargo fetch --locked` once. `specular-help`'s
licence test reads the whole dependency graph (`cargo metadata --offline`), which needs every
platform's crates downloaded, while a build fetches only the current platform's.

## Windows

Native Windows builds run `build.sh` from Git Bash or MSYS2 and need:

- Visual Studio 2022 or its Build Tools, with the "Desktop development with C++" workload (the
  MSVC compiler and linker, and a Windows SDK). Its "C++ CMake tools" component includes Ninja.
- LLVM, for `clang-cl` (`winget install LLVM.LLVM`). libjxl only builds with clang on Windows.
- Ninja, unless Visual Studio's copy is installed (`winget install Ninja-build.Ninja`).
- CMake (`winget install Kitware.CMake`), and for release builds NASM (`winget install NASM.NASM`).

`build.sh` looks for NASM, `clang-cl` and Ninja in their usual install folders when they are not
on `PATH`, and then builds libjxl with Ninja and `clang-cl` (`CMAKE_GENERATOR=Ninja`). Without
them it falls back to Visual Studio's "MSBuild support for LLVM (clang-cl) toolset" component, if
that is installed. A plain `cargo build` needs `CMAKE_GENERATOR=Ninja`, `ninja` and `clang-cl` on
`PATH`, or that component.

The C runtime is linked statically (`.cargo/config.toml`), so the binaries need nothing beyond
Windows itself: no Visual C++ Redistributable, whether installed by the MSI or copied as a folder.

The decode host (HEIC fallback, RAR and CBR) runs in an AppContainer. It starts from any folder
(`Program Files`, `target\`, an unzipped copy) without granting the folder to app packages:
Windows opens the program file as the viewer that starts it, and the host loads only system DLLs.
The host tests check full confinement that way from `target\`, and the Windows Sandbox test
(`packaging/windows/sandbox/run.ps1`) opens a CBR from an unzipped portable copy.

The Windows `.zip` is a portable copy: it carries the `portable` marker (settings and caches
stay in its folder) and a `README.txt` for whoever unzips it
(`packaging/windows/README-portable.txt`).

`./build.sh pack [FILE]` builds nothing: it packs the code and docs (the files git tracks or would
track, so no `target/`, corpora or other ignored files) into a source `.tgz`, by default
`/tmp/specular-<version>-src-<date>.tgz`. It packs the working tree, and warns when that has
uncommitted changes.

The binaries need an x86-64-v3 CPU (AVX2, from 2013/2015 onwards) on x86_64. Release Linux
packages should be built on an old distribution (the release workflow uses Ubuntu 22.04), because
the AppImage needs a glibc at least as new as the build machine's.
