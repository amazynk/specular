# Security policy

Specular opens files from anywhere, so parser bugs matter to us. Thank you for reporting them
responsibly.

## Reporting a vulnerability

Please **do not open a public issue**. Report it privately through
[GitHub's private vulnerability reporting](https://github.com/amazynk/specular/security/advisories/new),
or by email to [specular@amazynk.com](mailto:specular@amazynk.com) if you can't use GitHub.

Include the version, your OS, what an attacker could achieve, and a file or steps that reproduce
it. We will acknowledge the report within a few days, keep you updated while we fix it, and credit
you in the advisory unless you prefer otherwise.

## Scope

In scope: memory-safety bugs, sandbox escapes from the decode host, crashes or hangs from crafted
files, path traversal when extracting archives, and anything that makes Specular send data over the
network without the user asking.

Out of scope: problems that need an attacker who can already change your settings or binaries, and
bugs in libraries you install yourself (ODA File Converter, OpenCASCADE, libCZI, vendor SDKs);
please report those to their authors.

## Supported versions

Until 1.0, only the latest release gets security fixes.
