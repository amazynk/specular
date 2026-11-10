# Contributing to Specular

Thanks for helping. Bug reports, sample files, translations and pull requests are all welcome.

- **Questions and ideas:** [GitHub Discussions](https://github.com/amazynk/specular/discussions)
- **Chat:** [Discord](https://amazynk.com/discord/)
- **Bugs and format requests:** [issues](https://github.com/amazynk/specular/issues/new/choose)
- **Security problems:** privately, as described in [SECURITY.md](SECURITY.md), never in a public
  issue

## Ways to help

- **Files that don't open, or open wrong.** These are the most useful reports. Attach the file, or
  a small one that fails the same way, if you're allowed to share it.
- **Translations.** Specular speaks 19 languages, and some of them were machine-drafted. Native
  speakers reviewing a language help a lot. The catalogues are in `crates/specular-i18n/i18n/`,
  and each language has a glossary in [docs/i18n-glossary](docs/i18n-glossary).
- **Formats.** Each format has its own crate under `crates/formats/`. [HACKING.md](HACKING.md)
  explains how to add one.
- **Platforms.** Testing on hardware we don't have, such as particular GPUs, HDR displays,
  cameras for tethering, pen tablets and screen readers.

## Reporting a bug

Open an [issue](https://github.com/amazynk/specular/issues/new/choose) with:

- your OS and version, your GPU, and the Specular version (shown in the About window);
- what you did, what you expected and what happened;
- the file that fails, when you can share it. **Save diagnostics bundle…** (in the command palette
  or the About window) writes a zip you can attach. It lists everything it contains first, never
  includes images, passwords or server addresses, and can replace your file names.

## Pull requests

- For anything larger than a fix, open a discussion or an issue first, so we can agree on the
  approach before you spend time on it.
- Keep each pull request to one change, and say why it is needed.
- Add or update tests with the change. Decoders get a small fixture that exercises them.
- `cargo fmt`, `cargo clippy` and the tests should pass. CI runs them on Linux, Windows and macOS.
- If the UI changed on purpose, re-bless the screenshot goldens (see [HACKING.md](HACKING.md)) and
  include the new PNGs.
- User-visible text goes in `crates/specular-i18n/i18n/en/*.ftl`. Other languages may lag behind.

Building, testing, the repository layout and how a file opens are covered in
[HACKING.md](HACKING.md).

## Rules the code follows

The full set is in [docs/DESIGN.md](docs/DESIGN.md). The ones reviewers check most:

- **No `unsafe` without a reason.** `unsafe_code` is denied workspace-wide. An exception needs
  `#[allow(unsafe_code)]` on the smallest item, a `// SAFETY:` comment and an entry in
  [UNSAFE.md](UNSAFE.md).
- **Dependencies must be permissively licensed.** No GPL or AGPL, and no statically linked LGPL.
  `deny.toml` holds the allow list, and `cargo-deny` enforces it.
- **Untrusted C and C++ decoders run in the sandboxed decode host** (`specular-decode-host`), never
  in the viewer process.
- **Never look at the code of GPL-licensed viewers or editors** while writing an equivalent
  feature. Work from specifications, papers and observed behaviour.
- **No network access without the user asking for it.** Specular collects no telemetry.

## Licence

Specular is dual-licensed under the [MIT License](LICENSE-MIT) and the
[Apache License 2.0](LICENSE-APACHE). Unless you say otherwise, any contribution you submit for
inclusion in Specular, as defined in the Apache-2.0 license, is dual-licensed the same way, without
any additional terms or conditions.

Everyone taking part is expected to follow the [Code of Conduct](CODE_OF_CONDUCT.md).
