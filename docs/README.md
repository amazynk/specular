# Specular documentation

Using Specular is covered by its built-in manual: press <kbd>F1</kbd> in the app. The documents
here are for people who build on Specular, check what it supports, or work on its code.

## Using and integrating

| Document | What's in it |
|---|---|
| [ipc.md](ipc.md) | The remote-control protocol: drive a running Specular from scripts and other programs |
| [sidecar-format.md](sidecar-format.md) | The XMP sidecar Develop writes next to each photo |
| [metadata-coverage.md](metadata-coverage.md) | Which metadata Specular reads and writes, per format |
| [dicom-conformance.md](dicom-conformance.md) | How DICOM greyscale images are displayed |
| [develop-quality.md](develop-quality.md) | How close Develop's previews are to its exports, measured |
| [accessibility.md](accessibility.md) | Screen reader and input method support, tested per platform |

## Working on Specular

| Document | What's in it |
|---|---|
| [../HACKING.md](../HACKING.md) | Start here: building, testing, the repository layout and adding a format |
| [DESIGN.md](DESIGN.md) | The design: goals, architecture, formats and the rules the code follows |
| [building.md](building.md) | `build.sh`, toolchains, packaging and cross-compiling |
| [macos.md](macos.md) | macOS specifics: threads, file I/O, Metal and what is still to be measured |
| [i18n-glossary/](i18n-glossary) | Agreed terms for each language, for translators |
| [../tools/README.md](../tools/README.md) | Benchmarks, test corpora and other developer tools |
