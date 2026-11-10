## The manual's panel, crash reports, the diagnostics bundle, update checks and About.
## House style: short, plain, no filler; one idea per sentence.

## Palette

palette-help-manual = Help
palette-help-manual-help = Open the manual at the page for what is on screen.
palette-save-diagnostics = Save diagnostics bundle…
palette-save-diagnostics-help = Save crash reports, the log and system details to a zip file. Nothing is sent.
# A palette entry that opens a section of the manual; { $title } is a page or "Page › Section".
palette-help-topic = Help: { $title }
palette-help-topic-help = Open this section of the manual.
# Under an unavailable command in the palette.
palette-disabled-help = Unavailable now. Press Enter to read why.

## The help panel

help-title = Help
help-search-placeholder = Search the manual
help-contents = Contents
help-back = Back
help-forward = Forward
help-no-results = Nothing found for “{ $query }”.
help-results = { $count ->
    [one] 1 section found
   *[other] { $count } sections found
}
help-section-start = Getting started
help-section-modes = Viewing and modes
help-section-tasks = How do I…
help-section-reference = Reference
help-keys-unbound = Not set
# A command named in the manual that has no key; it runs from the palette.
help-key-unbound = { $name } (palette)
help-keys-command = Command
help-keys-keys = Keys
help-commands = Every command is in the command palette ({ $key }). Type part of its name.
help-commands-no-key = Every command is in the command palette. Type part of its name.
help-link = Go to { $title }
help-open = Help
# Screen reader name of the panel; { $page } is the page shown.
help-label = Help: { $page }
help-disabled-why = Why?

## Crash report offered at the next start

crash-title = { -app-name } closed unexpectedly
crash-body = A report was saved on this computer. Nothing was sent.
crash-view = View report
crash-hide = Hide report
crash-copy = Copy
crash-copied = Report copied
crash-save-bundle = Save diagnostics bundle…
crash-dismiss = Dismiss

## Diagnostics bundle

diag-title = Save diagnostics bundle
diag-intro = A zip file to attach to a bug report. It is saved on this computer. Nothing is sent.
diag-contents = Contents
diag-include-names = Include file names
diag-include-names-help = Off: names are replaced by file-1.jpg, dir-1 and so on.
diag-privacy = Your home folder shows as ~ and your user name as <user>. Passwords, server addresses and images are never included.
diag-where = Report a problem: { $issues } · { $email }
diag-save = Save…
diag-cancel = Cancel
diag-saved = Diagnostics bundle saved: { $path }
diag-failed = Couldn’t save the diagnostics bundle: { $reason }
diag-entry-summary = Version, system, GPU and driver
diag-entry-crash = Last crash report
diag-entry-crashes = Earlier crash reports ({ $count })
diag-entry-log = Last { $count } log lines
diag-entry-settings = Settings, without passwords or addresses
diag-entry-gpu = GPU and display details
diag-entry-readme = What this bundle is
diag-size = { $size }
# The README.txt inside the bundle.
diag-readme =
    { -app-name } diagnostics bundle.

    It holds what helps find a problem: version and system details, crash reports, the end of the log and the settings.

    Your home folder shows as ~ and your user name as <user>. Unless you chose to include them, file and folder names are replaced by file-1, dir-1 and so on. Passwords and server addresses are removed.

    Nothing was sent. You decide whether to share this file.

## Updates (Settings)

settings-section-updates = Updates
settings-updates-check = Check for updates
settings-updates-check-help = Once a day, ask whether a new version exists. The request carries only the version number: no identifier, no cookie. Off: { -app-name } never connects for updates.
settings-updates-channel = Channel
settings-updates-channel-help = Stable: releases only. Beta: also test versions.
settings-updates-channel-stable = Stable
settings-updates-channel-beta = Beta
settings-updates-check-now = Check now
settings-updates-check-now-help = Check once now.
update-available = Version { $version } is available
update-notes = Release notes
update-download = Download
update-none = { -app-name } is up to date.
update-checking = Checking for updates…
update-failed = Couldn’t check for updates: { $reason }
update-last-checked = Last checked { $when }

## About

about-build = Build { $commit } · { $profile } · { $target }
about-build-no-commit = Build { $profile } · { $target }
about-help = Help
about-diagnostics = Save diagnostics bundle…
about-renderer = Renderer info…
about-licences-count = { $count ->
    [one] 1 third-party component
   *[other] { $count } third-party components
}
about-licence-group = { $licence } ({ $count })
about-contact = Issues: { $issues } · Support: { $email }
