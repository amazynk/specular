## Astronomy interop: DS9 regions, plate solving, SAMP, HiPS sky surveys.

palette-science-load-regions = Overlay DS9 regions…
palette-science-load-regions-help = Draw the shapes of a DS9 region file (.reg) on the image.
palette-science-export-regions = Save regions as DS9…
palette-science-export-regions-help = Save the measurements and the overlaid regions as a DS9 region file.
palette-science-edit-regions = Edit regions as measurements
palette-science-edit-regions-help = Turn the overlaid regions into measurements you can move and change.
palette-science-clear-regions = Remove the overlaid regions
palette-science-clear-regions-help = Remove the DS9 regions from the image.
palette-science-plate-solve = Plate solve
palette-science-plate-solve-help = Find the image's sky coordinates with ASTAP or astrometry.net. Run again to stop.
palette-science-save-plate-solution = Save plate solution
palette-science-save-plate-solution-help = Save the sky coordinates found beside the image, as a .wcs file.
palette-science-sky-survey = Sky survey background
palette-science-sky-survey-help = Show the next sky survey around the image, or none.
palette-science-samp-send-image = Send image to SAMP applications
palette-science-samp-send-image-help = Open this image in DS9, Aladin and the other applications on the SAMP hub.
palette-science-samp-send-position = Send position to SAMP applications
palette-science-samp-send-position-help = Point the other applications at the sky position under the pointer.

disabled-samp-off = Connect to a SAMP hub first (Settings › Integration).

settings-integration-samp = Connect to a SAMP hub
settings-integration-samp-help = Exchange images, tables and sky positions with DS9, TOPCAT, Aladin and other astronomy applications. Only a hub on this computer is used.
settings-integration-astap-path = ASTAP program
settings-integration-astap-path-help = The ASTAP plate solver, when it is not found by itself.
settings-integration-solve-field-path = astrometry.net program
settings-integration-solve-field-path-help = The solve-field program of astrometry.net, when it is not found by itself.

astro-regions-filter = DS9 regions
astro-regions-loaded = { $name }: { $count ->
        [one] { $count } region.
       *[other] { $count } regions.
    }
astro-regions-refused = { $count ->
        [one] { $count } line not read.
       *[other] { $count } lines not read.
    } First: line { $line }, { $reason }.
astro-regions-approximate = { $count ->
        [one] { $count } panda drawn as its inner and outer edges.
       *[other] { $count } pandas drawn as their inner and outer edges.
    }
astro-regions-none-here = { $name }: no region falls on this image.
astro-regions-failed = { $name } could not be read: { $reason }
astro-regions-cleared = Regions removed.
astro-regions-nothing = No regions or measurements to save.
astro-regions-saved = Saved { $name }.
astro-regions-skipped = { $count ->
        [one] { $count } shape off the sky coordinates was left out.
       *[other] { $count } shapes off the sky coordinates were left out.
    }
astro-regions-save-failed = The regions could not be saved: { $reason }
astro-regions-edited = { $count ->
        [one] { $count } region is now a measurement.
       *[other] { $count } regions are now measurements.
    }
astro-regions-no-wcs = Sky regions need an image with sky coordinates.

astro-solve-no-solver = No plate solver found. Install ASTAP or astrometry.net, or pick one in Settings › Integration.
astro-solve-running = Solving with { $solver }… { $seconds } s
astro-solve-stop = Stop
astro-solve-stopped = Plate solving stopped.
astro-solve-done = Solved: { $ra }, { $dec }, { $scale } per pixel.
astro-solve-failed = Plate solving failed: { $reason }
astro-solve-not-local = Only images in a folder on this computer can be solved.
astro-solve-none = No plate solution for this image yet.
astro-solve-saved = Saved { $name }. The image itself is unchanged.
astro-solve-sidecar = Sky coordinates from { $name }.

astro-samp-connected = Connected to the SAMP hub.
astro-samp-no-hub = SAMP: { $reason }. Trying again when a hub starts.
astro-samp-lost = The SAMP hub stopped. Trying again when one starts.
astro-samp-sent = { $count ->
        [one] Sent to { $count } application.
       *[other] Sent to { $count } applications.
    }
astro-samp-nobody = No application on the hub listens for this.
astro-samp-failed = SAMP: { $reason }
astro-samp-refused = SAMP: { $url } is not on this computer, so it is not opened.
astro-samp-pointed = Pointed at { $ra }, { $dec }.
astro-samp-off-image = { $ra }, { $dec } is not on this image.
astro-samp-no-table = SAMP: that table is not overlaid here.
astro-samp-rows = { $count ->
        [one] { $count } catalogue source highlighted.
       *[other] { $count } catalogue sources highlighted.
    }

astro-survey-off = Sky survey off.
astro-survey-on = Sky survey: { $name }. { $credit }
astro-survey-failed = { $name } cannot be shown: { $reason }
astro-survey-credit = { $name }: { $credit }

net-what-sky-survey = Sky survey tiles from { $host }
