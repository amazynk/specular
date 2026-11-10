---
id: diagnostics
title: Crash reports and diagnostics
section: reference
contexts: crash, diagnostics
keywords: crash, report, bug, diagnostics, bundle, support, log, problem, issue, backtrace, renderer, gpu, driver, adapter, hdr
---
Nothing is ever sent. You choose whether to share a report, and how.

## After a crash {#crash}
If Specular closed unexpectedly, the next start offers the crash report:

- **View report** shows it.
- **Copy** copies it, to paste into a bug report or email.
- **Save diagnostics bundle…** saves it with more details.
- **Dismiss** closes the offer. The report stays in the crash folder for a while, then is deleted.

A report holds the Specular version, your operating system, the GPU and driver, where the crash happened, the last log lines and your settings.

Your home folder is replaced by `~` and your user name by `<user>`. Passwords are never included.

## Diagnostics bundle {#bundle}
1. Press [[key:save-diagnostics]], or choose **Save diagnostics bundle…** in the palette or in About.
2. Check the list of files the bundle will contain.
3. Tick **Include file names** only if they help explain the problem. Otherwise they are replaced by `file-1.jpg` and so on.
4. Choose **Save…** and a place for the zip file.
5. Attach it to an issue on the issue tracker, or send it to the support address shown in the dialog.

## Renderer information {#renderer}
**Renderer info…** in About, or Settings › Advanced › Renderer information…, shows:

- the graphics adapter, its driver and the graphics API;
- the limits and features Specular uses;
- the window's format, present mode and HDR;
- the memory budgets;
- the optional decoders found.

**Copy report** copies it as text, to paste into a bug report. Nothing is sent.

Settings › Advanced › Hardware report… shows the CPU and GPU features found and the fast paths used.

## Logs {#logs}
Specular writes its log to standard error and to `specular.log` in its log folder. Past a few MB the file is rotated; two old files are kept.

- Settings › Advanced › Log detail sets how much it writes.
- The `SPECULAR_LOG_FILE` environment variable names another file. Set it empty to write no file.
