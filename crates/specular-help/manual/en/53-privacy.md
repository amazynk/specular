---
id: privacy
title: Privacy and the network
section: reference
contexts: privacy, updates
keywords: privacy, yt-dlp, network, internet, offline, no network, air-gapped, telemetry, tracking, updates, version, release, gps, data, policy, administrator, firewall
---
Specular works entirely offline. It collects nothing and sends nothing: there is no telemetry of any kind.

## When Specular uses the network {#network}
Only when you ask it to:

- **Update checks**, if you turn them on.
- **Local AI downloads**, when you choose one in Settings › Local AI.
- **Servers you add**, such as DICOM or OMERO servers, when you open images from them, and the server or bucket of a Zarr address you open.
- **Base maps and map services**, if you turn them on.
- **Network streams** you open, from their server. A video page goes to yt-dlp only if you turned it on.

Place names for GPS positions come from data built into the app. Positions never leave your computer.

## No network {#no-network}
Settings › Privacy › **No network** stops Specular from connecting to anything.

- Features that need a server say “Network is off in Settings › Privacy” instead.
- Files on this computer and on mounted drives still open.
- Map tiles already downloaded still show.
- Local AI models can still be installed from files; see [Local AI](ai#offline).

For one run, start Specular with `--no-network`, or set `SPECULAR_NO_NETWORK=1`.

## Managed installs {#managed}
Administrators can set a policy for every user of a computer in one file:

- Linux: `/etc/specular/policy.toml`
- macOS: `/Library/Application Support/Specular/policy.toml`
- Windows: `%ProgramData%\Specular\policy.toml`

```toml
no_network = true
model_folder = "/srv/specular/models"
```

- `no_network = true` turns the network off. Users can’t turn it back on.
- `model_folder` names a folder of Local AI model files. Specular installs them from there instead of downloading.

Model files placed in `models` next to the policy file are used too, without a policy.

## Updates {#updates}
Specular checks for updates only if you turn it on in Settings › Updates.

- It checks at most once a day.
- The request carries no identifier, no cookie, and nothing about your computer except the Specular version.
- **Stable** offers released versions only. **Beta** also offers test versions.
- When a newer version exists, Specular links to its release notes and download page in your browser.

Nothing is downloaded or installed by itself.

## Crash reports {#crash-reports}
Crash reports stay on your computer. See [Crash reports and diagnostics](diagnostics).
