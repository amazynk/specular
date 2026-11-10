## Which kind of file? Files whose extension several formats share.

file-kind-title = Which kind of file is { $file }?
file-kind-why-content = Its content fits more than one format.
file-kind-why-name = Its content does not say. These formats use .{ $ext } files.
file-kind-why-none = No format recognises its content.
file-kind-certain = Certain
file-kind-likely = Likely
file-kind-possible = Possible
file-kind-guess = By name only
file-kind-hint-same = like other .{ $ext } files here
file-kind-hint-companions = its usual files are beside it
file-kind-scope = Use it for
file-kind-scope-file = This file
file-kind-scope-folder = .{ $ext } files in this folder
file-kind-scope-everywhere = All .{ $ext } files
file-kind-later = Change it later with Open as… in the file’s menu.
file-kind-open = Open
file-kind-raw = Raw samples
file-kind-open-as-none = Nothing else reads { $file }.
error-reason-ambiguous = It may be one of several formats. Choose one to open it.

## Open as… (the palette and the gallery's file menu)

palette-open-as = Open as…
palette-open-as-help = Choose which format to read the file as.

## Settings › Optional decoders › File kinds

settings-decoders-file-kinds = Forget all file kinds…
settings-decoders-file-kinds-help = Formats you chose for extensions several formats share. Asked again next time.
file-kind-rules-intro = Each rule opens files of an extension as one format.
file-kind-rules-none = No rules yet. Choose “.ext files in this folder” or “All .ext files” when asked.
file-kind-rule = .{ $ext } files: { $format }
file-kind-rule-folder = In { $folder }
file-kind-rule-everywhere = Everywhere
file-kind-forget = Forget…
file-kind-forget-ask = Forget “.{ $ext } files: { $format }”?
file-kind-forget-detail = { -app-name } asks again next time. No file is changed or removed.
file-kind-forget-ok = Forget
file-kind-forgotten = Forgot .{ $ext } files: { $format }.
file-kind-forget-all-ask = { $count ->
    [one] Forget the file kind rule?
   *[other] Forget all { $count } file kind rules?
}
file-kind-forgotten-all = { $count ->
    [one] Forgot the file kind rule.
   *[other] Forgot { $count } file kind rules.
}
file-kind-keywords = extension, format, open as, which kind, file type

## One line on what each format is, under its name in the prompt.

file-kind-about-other = Another format that uses .{ $ext } files.
file-kind-about-raw-volume = Headerless samples. You give width, height, slices and sample type.
file-kind-about-raw = Camera raw photo.
file-kind-about-flir-fff = FLIR thermal image with temperatures.
file-kind-about-fits = Astronomy image (FITS).
file-kind-about-fit = Garmin activity track: GPS points and sensor data.
file-kind-about-mif = MRtrix medical volume (MRI).
file-kind-about-map-info-mif = MapInfo vector map, with its .mid data.
file-kind-about-net-cdf = Scientific gridded data (climate, ocean).
file-kind-about-hdf5 = HDF5 scientific data.
file-kind-about-playlist = Audio playlist (PLS).
file-kind-about-gerber = Circuit board layer (Gerber), such as solder paste.
file-kind-about-ser = Astronomy video from a planetary camera (SER).
file-kind-about-tia = Electron microscope series (FEI TIA).
file-kind-about-pcd = 3D point cloud (PCD).
file-kind-about-photo-cd = Kodak Photo CD scan.
file-kind-about-zx-screen = ZX Spectrum screen dump.
file-kind-about-cpc-screen = Amstrad CPC screen dump.
file-kind-about-gimp-brush = GIMP brush.
file-kind-about-swatch-exchange = Adobe Swatch Exchange colour palette.
file-kind-about-aseprite = Aseprite pixel-art sprite.
file-kind-about-msx-screen = MSX screen dump.
file-kind-about-scc = Scenarist closed captions.
file-kind-about-pict = Classic Mac picture (PICT).
file-kind-about-hdr = Radiance HDR image.
file-kind-about-analyze = Analyze 7.5 medical volume, with its .hdr.
file-kind-about-envi = Remote-sensing raster (ENVI), with its .hdr.
file-kind-about-pds3 = Planetary mission image (PDS3), with its label.
file-kind-about-mpeg-ps = Camcorder video (MPEG program stream, MOD/TOD).
file-kind-about-tracker = Tracker music module (MOD, S3M, XM, IT, 669, MTM, STM, OKT, MED, ULT, FAR).
file-kind-about-softimage-pic = Softimage 3D render (PIC).
file-kind-about-dr-halo = Dr. Halo picture, with its .pal palette.
file-kind-about-wal = Quake 2 texture, coloured by the game’s palette.
file-kind-about-quick-time = QuickTime movie.
file-kind-about-pro-res-raw = Apple ProRes RAW video: camera sensor frames.
file-kind-about-cinema-raw-light = Canon Cinema RAW Light video: camera sensor frames.
