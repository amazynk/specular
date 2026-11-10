# Integration: open with, user commands, paste (Stage 2).
palette-open-with = Open with…
palette-paste = Paste
open-with-placeholder = Open with…
open-with-loading = Looking for applications…
open-with-default = { $name } (default)
open-with-choose-other = Choose another application…
open-with-choose-title = Choose an application
open-with-failed = Could not open with { $name }: { $reason }
command-missing = { $program } not found
command-failed = { $name } could not start: { $reason }
command-exited = { $name } ended with an error: { $status }
paste-nothing = Nothing to open on the clipboard
paste-failed = Could not paste the image: { $reason }
drag-failed = Could not drag the file: { $reason }
about-storage = Settings: { $settings } · Cache: { $cache }
about-storage-portable = Portable mode: settings and cache in { $dir }

## Scripting and recipes

palette-copy-view-command = Copy view as shell command
palette-copy-view-python = Copy view as Python
toast-copied-view-command = Command copied
script-copy-no-image = No image to describe
recipe-confirm-title = Run the recipe “{ $name }” for the first time? It runs: { $command }
recipe-confirm-run = Run
recipe-no-file = The recipe needs an image file
recipe-running = Running { $name }…
recipe-done = { $name }: { $shapes ->
    [one] 1 shape
   *[other] { $shapes } shapes
} added
recipe-failed = { $name } failed: { $reason }
recipe-timeout = stopped after { $seconds } s
recipe-no-output = { $name } wrote no { $file }
