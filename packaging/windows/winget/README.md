# winget manifest template (UNTESTED)

Multi-file manifest (schema 1.9) for `Specular.Specular`. Before submitting to
https://github.com/microsoft/winget-pkgs:

1. Replace `{{VERSION}}`, `{{MSI_URL}}`, `{{MSI_SHA256}}` and `{{PRODUCT_CODE}}` (the MSI's
   ProductCode, from `wix msi decompile` or Orca), and move the files to
   `manifests/s/Specular/Specular/{{VERSION}}/`.
2. Validate: `winget validate --manifest <dir>`; test: `winget install --manifest <dir>`.

`packaging/windows/build-msi.ps1` writes the SHA-256 next to the MSI.
