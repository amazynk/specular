# Developing Specular

This guide is for people working on Specular's code. For how to report bugs and send changes, see
[CONTRIBUTING.md](CONTRIBUTING.md). For the full design and the reasons behind it, see
[docs/DESIGN.md](docs/DESIGN.md).

## Quick start

You need [Rust](https://rustup.rs), a C/C++ compiler and CMake. The nightly toolchain pinned in
`rust-toolchain.toml` installs itself the first time you run `cargo`.

```sh
git clone https://github.com/amazynk/specular.git && cd specular
cargo run -p specular -- ~/Pictures   # a debug build, opened on a folder
cargo fetch --locked                  # once, before the first test run
cargo test --workspace
```

Debug builds compile Specular's own crates at `opt-level = 1` and every dependency at `3`, so they
are fast enough to use day to day. `./build.sh` makes optimised release builds and packages; its
options, the Windows and macOS toolchains and cross-compiling are in
[docs/building.md](docs/building.md).

## Repository layout

| Path | What's there |
|---|---|
| [`app/specular`](app/specular) | The application: windows, modes, input handling, the command line and platform integration |
| [`crates/specular-core`](crates/specular-core) | Memory-mapped sources, documents, planar rasters, the prioritised job system |
| [`crates/specular-decode`](crates/specular-decode) | The `Decoder` trait, the format registry and probing, and one adapter per format family |
| [`crates/formats/`](crates/formats) | About 140 format crates: parsers and codecs, each with its own tests and fixtures |
| `crates/specular-gfx`, `-widgets`, `-theme` | Specular's own UI toolkit on wgpu: drawing, text, widgets, design tokens |
| `crates/specular-render`, `-render-3d`, `-render-volume` | Images and tiled pyramids, meshes and point clouds, volumes |
| `crates/specular-color`, `-ocio` | ICC colour management and OpenColorIO configs |
| `crates/specular-develop` | Develop's edit model, sidecars and GPU render graph |
| `crates/specular-library`, `-thumbs`, `-query` | The catalogue, the thumbnail engine and search |
| `crates/specular-sandbox`, `-decode-host` | The sandboxed helper process that runs C and C++ libraries |
| `crates/specular-ai`, `-infer` | Local AI: the model manager and a small inference runtime of its own |
| `crates/specular-i18n`, `-help` | Translations (Fluent) and the offline manual |
| [`clients/python`](clients/python) | Python client for the [IPC protocol](docs/ipc.md) |
| [`packaging/`](packaging) | Installers for each platform and the third-party notices |
| [`tools/`](tools) | Benchmarks, test corpora, quality oracles, fuzzing and release scripts ([tools/README.md](tools/README.md)) |
| [`third_party/`](third_party) | Third-party crates Specular patches (UnRAR, resvg, libjxl's build), with the patches |

Every crate's `Cargo.toml` has a one-line description of what it does.

## How a file opens

1. **Identify.** `specular-decode`'s registry asks each decoder whether the file is its format,
   by content first. The extension only breaks ties between formats that share one
   (`crates/specular-decode/src/ident.rs`).
2. **Open a session.** The decoder returns a `Session`, which answers cheap questions (size,
   orientation, colour profile, embedded preview) without decoding, and decodes any scale or
   region on request. Decoders read from a `Source`, never a path, so the same code works on
   files, archive members and remote objects.
3. **Draw.** `specular-render` uploads the planes and converts them to linear light on the GPU,
   with colour management from `specular-color`. A small overview appears first, then the tiles
   nearest the view, then the rest.
4. **Sandbox.** Decoders that use C or C++ libraries run in `specular-decode-host`, a separate
   locked-down process, through `specular-sandbox`. A crash there costs one file, not the window.

## Adding a file format

1. **Parser.** Write it in a crate under `crates/formats/`, or extend an existing one. It works on
   bytes, bounds every allocation by what the file can actually hold, and has no UI.
2. **Decoder.** In `specular-decode`, implement `Decoder` (`probe`, `extensions`, `open`), add a
   variant to `Format` and register the decoder in `Registry`. If the extension is shared or the
   format has no signature, implement `confidence` (see `ident.rs`).
3. **Desktop registration.** Add each extension and its MIME type to
   `crates/specular-decode/src/mime.rs`. A test fails when one is missing.
4. **Tests.** Add small fixtures to the crate's `tests/data/`, with where each came from. Where a
   reference decoder exists, compare against it: the oracle scripts in `tools/corpus/` show how.
5. **Fuzzing.** Add a `cargo-fuzz` target under the crate's `fuzz/` and run it with
   `tools/fuzz/run.sh crates/formats/<crate> 60`.

Never read the code of GPL-licensed programs while writing a decoder; work from specifications,
papers and sample files. Data copied from elsewhere (tables, codebooks) needs a compatible licence,
a `PROVENANCE.md` next to it and an entry in `tools/release/third-party.sh`.

## Testing

- **Unit and integration tests:** `cargo test --workspace`. Long or machine-specific tests are
  `#[ignore]`d and say how to run them.
- **Screenshot tests** compare the UI with PNG goldens in `*/tests/shots/`. After an intended UI
  change, regenerate them with `SPECULAR_BLESS=1 cargo test -p <crate> --test shots` and commit
  the new PNGs with the change.
- **Corpora and oracles:** larger public test files are downloaded, never committed, by the
  `tools/corpus/fetch-*.sh` scripts, and checked against reference decoders. See
  [tools/README.md](tools/README.md) and [tools/quality/README.md](tools/quality/README.md).
- **Speed:** CI counts instructions for a set of headline cases and fails when one gets slower
  (`tools/bench/perf-gate.py`).
- **Licences:** `tools/ci/deny.sh` checks every dependency's licence and advisories.
  `tools/release/third-party.sh --check` checks that `packaging/THIRD-PARTY.md` is current.

## Debugging

- `SPECULAR_LOG` (or `--log`) turns on logging by component: `SPECULAR_LOG=decode,tiles` for debug
  output from two components, `SPECULAR_LOG=all:info,gpu:trace` for more. `SPECULAR_LOG_FILE`
  writes it to a file.
- `specular --hw-report` prints what Specular found out about the CPU, GPU and display.
- **Save diagnostics bundle…** in the command palette writes the zip users attach to bug reports.
  It lists its contents before saving and never includes images.

## Code conventions

- `cargo fmt` and `cargo clippy --workspace --all-targets -- -D warnings` are clean.
- `unsafe` is denied workspace-wide. An exception needs `#[allow(unsafe_code)]` on the smallest
  item, a `// SAFETY:` comment and an entry in [UNSAFE.md](UNSAFE.md).
- User-visible text goes in the English Fluent catalogues in `crates/specular-i18n/i18n/en/`.
  English text uses British spelling (colour, catalogue), and other languages follow the
  glossaries in [docs/i18n-glossary](docs/i18n-glossary).
- Dependencies must be permissively licensed: no GPL or AGPL, and no statically linked LGPL.
  `deny.toml` holds the list.
- Nothing touches the network unless the user asked for it.

## Releases

Pushing a `v*` tag runs `.github/workflows/release.yml`. It builds and packages for each platform,
signs when the signing secrets are set, and drafts a GitHub release. Update
[CHANGELOG.md](CHANGELOG.md) first.
