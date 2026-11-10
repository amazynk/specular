//! MIME types of the file extensions the decoders claim, for desktop registration (the
//! `MimeType=` lists, the shared-MIME-info package and the thumbnailer). Names follow the
//! freedesktop shared-MIME-info database where it has the type, so our registration merges
//! with the system's instead of inventing aliases.

/// (extension, MIME type). `None` marks an extension we decode but deliberately do not
/// register because the system database (or common use) gives it to something else:
/// `.rs` (Rust source), `.vst` (Visio stencils), `.pic`,
/// `.bw`/`.int`/`.inta` (too generic), `.mpo` and `.svs` (no type of their own). Every
/// extension of every decoder must be listed (a test checks it).
pub const EXTENSION_MIME: &[(&str, Option<&str>)] = &[
    // JPEG
    ("jpg", Some("image/jpeg")),
    ("jpeg", Some("image/jpeg")),
    ("jpe", Some("image/jpeg")),
    ("jfif", Some("image/jpeg")),
    // Camera sidecars that are JPEG: GoPro / Canon thumbnails, Insta360 photos.
    ("thm", Some("image/jpeg")),
    ("insp", Some("image/jpeg")),
    ("jif", Some("image/jpeg")),
    // Multi-Picture JPEG (stereo cameras, `.mpo`): no shared-MIME-info type; the name
    // ExifTool and image tools use, with the x- prefix.
    ("mpo", Some("image/x-mpo")),
    // PNG
    ("png", Some("image/png")),
    ("apng", Some("image/apng")),
    // WebP, AVIF, JPEG XL
    ("webp", Some("image/webp")),
    ("avif", Some("image/avif")),
    ("avifs", Some("image/avif")),
    ("jxl", Some("image/jxl")),
    // BPG and FLIF (their authors' types; not in shared-MIME-info)
    ("bpg", Some("image/bpg")),
    ("flif", Some("image/flif")),
    // JPEG 2000 / HTJ2K
    ("jp2", Some("image/jp2")),
    ("j2k", Some("image/x-jp2-codestream")),
    ("j2c", Some("image/x-jp2-codestream")),
    ("jpc", Some("image/x-jp2-codestream")),
    ("jls", Some("image/jls")),
    // FlashPix (IANA's registered type).
    ("fpx", Some("image/vnd.fpx")),
    // JPEG XR (shared-MIME-info's name; image/vnd.ms-photo is its alias)
    ("jxr", Some("image/jxr")),
    ("wdp", Some("image/jxr")),
    ("hdp", Some("image/jxr")),
    ("jpf", Some("image/jpx")),
    ("jpx", Some("image/jpx")),
    ("jph", Some("image/jph")),
    ("jhc", Some("image/jphc")),
    // JPEG 2000 Part 6 compound images (shared-MIME-info's name)
    ("jpm", Some("image/jpm")),
    ("jpgm", Some("image/jpm")),
    // HEIF
    ("heic", Some("image/heic")),
    ("heif", Some("image/heif")),
    ("hif", Some("image/heif")),
    // HEIF image sequences (IANA type).
    ("heics", Some("image/heic-sequence")),
    // Classic bitmaps
    ("gif", Some("image/gif")),
    ("bmp", Some("image/bmp")),
    ("dib", Some("image/bmp")),
    ("ico", Some("image/vnd.microsoft.icon")),
    ("cur", Some("image/x-win-bitmap")),
    ("ani", Some("application/x-navi-animation")),
    ("qoi", Some("image/qoi")),
    ("pnm", Some("image/x-portable-anymap")),
    ("pbm", Some("image/x-portable-bitmap")),
    ("pgm", Some("image/x-portable-graymap")),
    ("ppm", Some("image/x-portable-pixmap")),
    ("pam", Some("image/x-portable-arbitrarymap")),
    ("tga", Some("image/x-tga")),
    ("icb", Some("image/x-tga")),
    ("vda", Some("image/x-tga")),
    ("vst", None),
    // SVG
    ("svg", Some("image/svg+xml")),
    ("svgz", Some("image/svg+xml-compressed")),
    // PDF, Illustrator, EPS (previews only)
    ("pdf", Some("application/pdf")),
    ("ai", Some("application/illustrator")),
    ("eps", Some("image/x-eps")),
    ("epsf", Some("image/x-eps")),
    ("epsi", Some("image/x-eps")),
    // PostScript documents open only with an EPS preview inside: not registered.
    ("ps", None),
    // DjVu (shared-mime-info: image/vnd.djvu, alias image/x-djvu), XPS, OpenXPS, CGM
    ("djvu", Some("image/vnd.djvu")),
    ("djv", Some("image/vnd.djvu")),
    ("xps", Some("application/vnd.ms-xpsdocument")),
    ("oxps", Some("application/oxps")),
    ("cgm", Some("image/cgm")),
    // Fonts (shared-mime-info's font/* types) and EPUB books
    ("ttf", Some("font/ttf")),
    ("otf", Some("font/otf")),
    ("ttc", Some("font/collection")),
    ("otc", Some("font/collection")),
    ("woff", Some("font/woff")),
    ("woff2", Some("font/woff2")),
    // PostScript Type 1 (shared-MIME-info's type).
    ("pfb", Some("application/x-font-type1")),
    ("pfa", Some("application/x-font-type1")),
    // TeX DVI (shared-MIME-info's type).
    ("dvi", Some("application/x-dvi")),
    ("epub", Some("application/epub+zip")),
    // Kindle and Mobipocket books, FictionBook (shared-MIME-info names).
    ("mobi", Some("application/x-mobipocket-ebook")),
    ("prc", Some("application/x-mobipocket-ebook")),
    ("azw", Some("application/vnd.amazon.ebook")),
    ("azw3", Some("application/vnd.amazon.mobi8-ebook")),
    ("fb2", Some("application/x-fictionbook+xml")),
    ("fb2.zip", Some("application/x-zip-compressed-fb2")),
    // DICOM
    ("dcm", Some("application/dicom")),
    ("dicom", Some("application/dicom")),
    // Camera RAW
    ("dng", Some("image/x-adobe-dng")),
    ("cr2", Some("image/x-canon-cr2")),
    ("cr3", Some("image/x-canon-cr3")),
    ("nef", Some("image/x-nikon-nef")),
    ("nrw", Some("image/x-nikon-nrw")),
    ("arw", Some("image/x-sony-arw")),
    ("srf", Some("image/x-sony-srf")),
    ("sr2", Some("image/x-sony-sr2")),
    ("raf", Some("image/x-fuji-raf")),
    ("orf", Some("image/x-olympus-orf")),
    ("rw2", Some("image/x-panasonic-rw2")),
    ("rwl", Some("image/x-leica-rwl")),
    ("pef", Some("image/x-pentax-pef")),
    ("srw", Some("image/x-samsung-srw")),
    ("crw", Some("image/x-canon-crw")),
    ("mrw", Some("image/x-minolta-mrw")),
    ("x3f", Some("image/x-sigma-x3f")),
    ("dcr", Some("image/x-kodak-dcr")),
    ("kdc", Some("image/x-kodak-kdc")),
    // Not in shared-MIME-info: our own names (packaging/linux/app.specular.Specular.mime.xml).
    ("3fr", Some("image/x-hasselblad-3fr")),
    ("iiq", Some("image/x-phaseone-iiq")),
    ("gpr", Some("image/x-gopro-gpr")),
    ("mos", Some("image/x-leaf-mos")),
    ("mef", Some("image/x-mamiya-mef")),
    ("erf", Some("image/x-epson-erf")),
    ("ari", Some("image/x-arri-ari")),
    // Older and rarer cameras (our own names). `.ori` is Olympus's high-resolution ORF.
    ("ori", Some("image/x-olympus-orf")),
    ("sti", Some("image/x-sinar-sti")),
    ("k25", Some("image/x-kodak-k25")),
    ("qtk", Some("image/x-quicktake")),
    ("mdc", Some("image/x-minolta-mdc")),
    ("j6i", Some("image/x-ricoh-j6i")),
    ("lfp", Some("image/x-lytro-lfp")),
    ("lfr", Some("image/x-lytro-lfp")),
    ("eip", Some("image/x-phaseone-eip")),
    // Rawzor, recognised only to refuse in plain words.
    ("rwz", None),
    // Canon Cinema RAW Light clips (own CRX reader): our own name.
    ("crm", Some("video/x-canon-crm")),
    // RED R3D, recognised only to refuse in plain words: not registered.
    ("r3d", None),
    // Blackmagic RAW clips (through the user's Blackmagic RAW SDK): our own name.
    ("braw", Some("video/x-braw")),
    // TIFF, textures, icons, Photoshop
    ("tif", Some("image/tiff")),
    ("tiff", Some("image/tiff")),
    // Microscopy stacks in TIFF containers, in no MIME database: ours are sub-classes of
    // image/tiff. `.gel` (Molecular Dynamics) is too generic a name to claim.
    ("lsm", Some("image/x-zeiss-lsm")),
    ("stk", Some("image/x-metamorph-stk")),
    ("gel", None),
    ("dds", Some("image/x-dds")),
    ("icns", Some("image/x-icns")),
    ("ktx", Some("image/ktx")),
    ("ktx2", Some("image/ktx2")),
    ("pvr", Some("image/x-pvr")),
    ("vtf", Some("image/x-vtf")),
    ("astc", Some("image/astc")),
    ("basis", Some("image/x-basis")),
    // PKM (etcpack's ETC files) and Firaxis textures: not in shared-MIME-info; ours.
    ("pkm", Some("image/x-pkm")),
    ("ftc", Some("image/x-firaxis-texture")),
    ("ftu", Some("image/x-firaxis-texture")),
    ("psd", Some("image/vnd.adobe.photoshop")),
    ("psb", Some("image/vnd.adobe.photoshop")),
    // PhotoDeluxe and Photoshop templates: plain PSD.
    ("pdd", Some("image/vnd.adobe.photoshop")),
    ("psdt", Some("image/vnd.adobe.photoshop")),
    // Painting documents (merged image)
    ("ora", Some("image/openraster")),
    ("kra", Some("application/x-krita")),
    ("krz", Some("application/x-krita")),
    ("procreate", Some("application/x-procreate")),
    ("afphoto", Some("application/x-affinity-photo")),
    ("afdesign", Some("application/x-affinity-designer")),
    ("afpub", Some("application/x-affinity-publisher")),
    ("pdn", Some("image/x-paintnet")),
    ("psp", Some("image/x-paintshoppro")),
    ("pspimage", Some("image/x-paintshoppro")),
    ("tub", Some("image/x-paintshoppro")),
    ("psptube", Some("image/x-paintshoppro")),
    ("pspframe", Some("image/x-paintshoppro")),
    ("pspmask", Some("image/x-paintshoppro")),
    ("cdr", Some("application/vnd.corel-draw")),
    // Clip Studio Paint: not in shared-MIME-info.
    ("clip", Some("application/x-clip-studio")),
    // Visio (shared-MIME-info names); `.vst` is a Targa extension here, `.vdx` is refused.
    ("vsdx", Some("application/vnd.ms-visio.drawing.main+xml")),
    (
        "vsdm",
        Some("application/vnd.ms-visio.drawing.macroEnabled.main+xml"),
    ),
    ("vstx", Some("application/vnd.ms-visio.template.main+xml")),
    (
        "vstm",
        Some("application/vnd.ms-visio.template.macroEnabled.main+xml"),
    ),
    ("vssx", Some("application/vnd.ms-visio.stencil.main+xml")),
    (
        "vssm",
        Some("application/vnd.ms-visio.stencil.macroEnabled.main+xml"),
    ),
    ("vsd", Some("application/vnd.visio")),
    ("vss", Some("application/vnd.visio")),
    ("vdx", None),
    // WordPerfect Graphics
    ("wpg", Some("application/x-wpg")),
    // Sketch and Figma: not in shared-MIME-info.
    ("sketch", Some("application/x-sketch")),
    ("fig", Some("application/x-figma")),
    // Office documents, by their stored preview (shared-MIME-info names). `.dot` and `.pot`
    // are not registered: Graphviz and others use them too.
    (
        "docx",
        Some("application/vnd.openxmlformats-officedocument.wordprocessingml.document"),
    ),
    (
        "dotx",
        Some("application/vnd.openxmlformats-officedocument.wordprocessingml.template"),
    ),
    (
        "docm",
        Some("application/vnd.ms-word.document.macroEnabled.12"),
    ),
    (
        "dotm",
        Some("application/vnd.ms-word.template.macroEnabled.12"),
    ),
    (
        "xlsx",
        Some("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
    ),
    (
        "xltx",
        Some("application/vnd.openxmlformats-officedocument.spreadsheetml.template"),
    ),
    (
        "xlsm",
        Some("application/vnd.ms-excel.sheet.macroEnabled.12"),
    ),
    (
        "xltm",
        Some("application/vnd.ms-excel.template.macroEnabled.12"),
    ),
    (
        "xlsb",
        Some("application/vnd.ms-excel.sheet.binary.macroEnabled.12"),
    ),
    (
        "pptx",
        Some("application/vnd.openxmlformats-officedocument.presentationml.presentation"),
    ),
    (
        "potx",
        Some("application/vnd.openxmlformats-officedocument.presentationml.template"),
    ),
    (
        "ppsx",
        Some("application/vnd.openxmlformats-officedocument.presentationml.slideshow"),
    ),
    (
        "pptm",
        Some("application/vnd.ms-powerpoint.presentation.macroEnabled.12"),
    ),
    (
        "potm",
        Some("application/vnd.ms-powerpoint.template.macroEnabled.12"),
    ),
    (
        "ppsm",
        Some("application/vnd.ms-powerpoint.slideshow.macroEnabled.12"),
    ),
    ("doc", Some("application/msword")),
    ("dot", None),
    ("xls", Some("application/vnd.ms-excel")),
    ("xlt", Some("application/vnd.ms-excel")),
    ("ppt", Some("application/vnd.ms-powerpoint")),
    ("pps", Some("application/vnd.ms-powerpoint")),
    ("pot", None),
    ("pages", Some("application/vnd.apple.pages")),
    ("numbers", Some("application/vnd.apple.numbers")),
    ("key", Some("application/vnd.apple.keynote")),
    // OpenDocument (shared-MIME-info names): drawings and presentations drawn, the rest by
    // their thumbnail.
    ("odg", Some("application/vnd.oasis.opendocument.graphics")),
    (
        "otg",
        Some("application/vnd.oasis.opendocument.graphics-template"),
    ),
    (
        "fodg",
        Some("application/vnd.oasis.opendocument.graphics-flat-xml"),
    ),
    (
        "odp",
        Some("application/vnd.oasis.opendocument.presentation"),
    ),
    (
        "otp",
        Some("application/vnd.oasis.opendocument.presentation-template"),
    ),
    (
        "fodp",
        Some("application/vnd.oasis.opendocument.presentation-flat-xml"),
    ),
    (
        "ods",
        Some("application/vnd.oasis.opendocument.spreadsheet"),
    ),
    (
        "ots",
        Some("application/vnd.oasis.opendocument.spreadsheet-template"),
    ),
    ("odt", Some("application/vnd.oasis.opendocument.text")),
    (
        "ott",
        Some("application/vnd.oasis.opendocument.text-template"),
    ),
    ("xcf", Some("image/x-xcf")),
    ("xcf.gz", Some("image/x-compressed-xcf")),
    ("xcf.bz2", Some("image/x-compressed-xcf")),
    ("xcf.xz", Some("image/x-compressed-xcf")),
    ("xcfgz", Some("image/x-compressed-xcf")),
    ("xcfbz2", Some("image/x-compressed-xcf")),
    ("xcfxz", Some("image/x-compressed-xcf")),
    // Legacy formats
    ("ff", Some("image/x-farbfeld")),
    // Retro, game and terminal rasters. shared-mime-info has MacPaint (`.pntg`), QuickTime
    // images (`.qtif`, `.qif`) and image/rle for `.rle`; the others take x- names. `.blp` is
    // also GNOME Blueprint's text/x-blueprint: told apart by the "BLP1"/"BLP2" magic. `.scr`
    // (Windows screen savers) is not registered: a ZX Spectrum screen is recognised by its
    // size only. `.pic` (Softimage, Radiance) and GEM's `.img` (disk images) are registered
    // by magic only (CONTENT_MIME), as are X cursors, which have no extension.
    ("six", Some("image/x-sixel")),
    ("sixel", Some("image/x-sixel")),
    ("qtif", Some("image/x-quicktime")),
    ("qif", Some("image/x-quicktime")),
    ("avs", Some("image/x-avs")),
    ("pntg", Some("image/x-macpaint")),
    ("mac", Some("image/x-macpaint")),
    ("pi1", Some("image/x-degas")),
    ("pi2", Some("image/x-degas")),
    ("pi3", Some("image/x-degas")),
    ("pc1", Some("image/x-degas")),
    ("pc2", Some("image/x-degas")),
    ("pc3", Some("image/x-degas")),
    ("neo", Some("image/x-neochrome")),
    ("koa", Some("image/x-koala")),
    ("kla", Some("image/x-koala")),
    ("scr", None),
    ("cut", Some("image/x-halo-cut")),
    ("pix", Some("image/x-alias-pix")),
    ("rla", Some("image/x-rla")),
    ("rpf", Some("image/x-rla")),
    ("rle", Some("image/rle")),
    ("wal", Some("image/x-quake2-wal")),
    ("blp", Some("image/x-blp")),
    ("ras", Some("image/x-cmu-raster")),
    ("sun", Some("image/x-sun-raster")),
    ("rs", None),
    ("im1", Some("image/x-sun-raster")),
    ("im8", Some("image/x-sun-raster")),
    ("im24", Some("image/x-sun-raster")),
    ("im32", Some("image/x-sun-raster")),
    ("sgi", Some("image/x-sgi")),
    ("rgb", Some("image/x-rgb")),
    ("rgba", Some("image/x-sgi")),
    ("bw", None),
    ("int", None),
    ("inta", None),
    ("fli", Some("video/x-flic")),
    ("flc", Some("video/x-flic")),
    ("pcx", Some("image/vnd.zbrush.pcx")),
    ("dcx", Some("image/x-dcx")),
    ("mng", Some("video/x-mng")),
    ("jng", Some("image/x-jng")),
    ("wmf", Some("image/wmf")),
    ("emf", Some("image/emf")),
    // Gzip-compressed metafiles (Office, LibreOffice): not in shared-MIME-info; the names
    // LibreOffice uses.
    ("wmz", Some("image/x-wmz")),
    ("emz", Some("image/x-emz")),
    ("iff", Some("image/x-ilbm")),
    ("ilbm", Some("image/x-ilbm")),
    ("lbm", Some("image/x-ilbm")),
    // IFF ANIM (shared-MIME-info's type); DEEP, RGB8 and RGBN have none of their own.
    ("anim", Some("video/x-anim")),
    ("deep", None),
    ("rgb8", None),
    ("rgbn", None),
    // `pic` is Radiance's.
    ("pict", Some("image/x-pict")),
    ("pct", Some("image/x-pict")),
    ("xpm", Some("image/x-xpixmap")),
    // X Window dumps: shared-mime-info's type.
    ("xwd", Some("image/x-xwindowdump")),
    // 8-bit home computer screens (none in shared-MIME-info: our own names). `.scc` is also
    // `.scr` Windows screen savers (and ZX Spectrum screens, above): decoded (with a header
    // or at the screen's size) but not registered. `.scc` is Scenarist captions' (below).
    ("sc2", Some("image/x-msx-screen")),
    ("sc5", Some("image/x-msx-screen")),
    ("sc6", Some("image/x-msx-screen")),
    ("sc7", Some("image/x-msx-screen")),
    ("sc8", Some("image/x-msx-screen")),
    ("sca", Some("image/x-msx-screen")),
    ("hgr", Some("image/x-apple-ii-hires")),
    ("dhr", Some("image/x-apple-ii-double-hires")),
    ("a2fc", Some("image/x-apple-ii-double-hires")),
    ("bb0", Some("image/x-bbc-micro-screen")),
    ("bb1", Some("image/x-bbc-micro-screen")),
    ("bb2", Some("image/x-bbc-micro-screen")),
    ("bb4", Some("image/x-bbc-micro-screen")),
    ("bb5", Some("image/x-bbc-micro-screen")),
    ("mic", Some("image/x-atari-micropainter")),
    ("gr8", Some("image/x-atari-graphics8")),
    ("gr9", Some("image/x-atari-graphics9")),
    // ImageMagick's own format: not in shared-mime-info; ours, with the "id=ImageMagick"
    // magic. (`.mif` is MRtrix's and FrameMaker's.)
    ("miff", Some("image/x-miff")),
    // PlayStation TIM: not in shared-mime-info; ours.
    ("tim", Some("image/x-sony-tim")),
    // GIMP brushes, animated brushes and patterns (shared-MIME-info's types; `.gbr` is also
    // Gerber's, below, told apart by content). GIMP palettes and gradients, Photoshop
    // swatches, Adobe colour tables and Swatch Exchange: not in shared-MIME-info; ours.
    ("gih", Some("image/x-gimp-gih")),
    ("pat", Some("image/x-gimp-pat")),
    ("gpl", Some("image/x-gimp-gpl")),
    ("ggr", Some("image/x-gimp-ggr")),
    ("aco", Some("image/x-adobe-aco")),
    ("act", Some("image/x-adobe-act")),
    ("ase", Some("image/x-adobe-ase")),
    // Aseprite (its own name; `.ase` too, below).
    ("aseprite", Some("image/x-aseprite")),
    // Microsoft Paint, libgd, Pixar, LabEye IM, IM Tools, PGX and CALS: none is in
    // shared-MIME-info; ours. (`.gd` is also Godot's scripts: GD files are told apart by
    // content.)
    ("msp", Some("image/x-msp")),
    ("gd", Some("image/x-gd")),
    ("gd2", Some("image/x-gd2")),
    ("pxr", Some("image/x-pixar")),
    ("picio", Some("image/x-pixar")),
    ("im", Some("image/x-labeye-im")),
    ("imt", Some("image/x-imt")),
    ("pgx", Some("image/x-pgx")),
    ("cal", Some("image/x-cals")),
    ("cals", Some("image/x-cals")),
    ("ct1", Some("image/x-cals")),
    // FBI WSQ fingerprints: not in shared-mime-info; image/x-wsq is the name NIST tools and
    // others use.
    ("wsq", Some("image/x-wsq")),
    // JBIG (T.82) and standalone JBIG2 (T.88) files: neither is in shared-mime-info; ours,
    // JBIG2 with its file-header magic.
    ("jbg", Some("image/x-jbig")),
    ("jbig", Some("image/x-jbig")),
    ("bie", Some("image/x-jbig")),
    ("jb2", Some("image/x-jbig2")),
    ("jbig2", Some("image/x-jbig2")),
    // JPEG XS (ISO/IEC 21122-3): IANA's image/jxs, for codestreams and image files alike.
    ("jxs", Some("image/jxs")),
    // JPEG XS images in HEIF (`jxs1` items): IANA's image/jxsi.
    ("jxsi", Some("image/jxsi")),
    ("xbm", Some("image/x-xbitmap")),
    ("wbmp", Some("image/vnd.wap.wbmp")),
    // Film scans: DPX is shared-mime-info's image/dpx; Cineon is not there, ours.
    ("dpx", Some("image/dpx")),
    ("cin", Some("image/x-cineon")),
    // HDR
    ("exr", Some("image/x-exr")),
    ("hdr", Some("image/vnd.radiance")),
    ("rgbe", Some("image/vnd.radiance")),
    ("pic", None),
    // PFM is not in shared-mime-info; image/x-portable-floatmap is the name netpbm-family
    // tools use. The package adds it with the "PF"/"Pf" magic, so a printer font metrics file
    // that shares the extension keeps its own type.
    ("pfm", Some("image/x-portable-floatmap")),
    // Astronomy. shared-mime-info's FITS type (application/fits is its alias) also has magic
    // for "SIMPLE  =". fpack's tile-compressed `.fz` files are in no MIME database: ours is
    // image/x-fpack, a sub-class of image/fits (by name; the magic is FITS's). XISF is not
    // in shared-mime-info; PixInsight registers image/x-xisf.
    ("fits", Some("image/fits")),
    ("fit", Some("image/fits")),
    ("fts", Some("image/fits")),
    // Whole-file gzip, as archives (MAST, ESO, SDSS) serve FITS: FITS's type by name.
    ("fits.gz", Some("image/fits")),
    ("fit.gz", Some("image/fits")),
    ("fts.gz", Some("image/fits")),
    ("fz", Some("image/x-fpack")),
    ("fits.fz", Some("image/x-fpack")),
    // Planetary images: in no MIME database. VICAR is image/x-vicar (magic "LBLSIZE=");
    // PDS3 detached labels are application/x-pds (magic "PDS_VERSION_ID"; a label, so not
    // claimed by the default-viewer option; `.lbl` also names label-printer files, told apart
    // by content). Attached PDS3 and VICAR `.img` files and PDS4 `.xml` labels open by
    // content only.
    ("vic", Some("image/x-vicar")),
    ("vicar", Some("image/x-vicar")),
    ("lbl", Some("application/x-pds")),
    ("xisf", Some("image/x-xisf")),
    // ASDF (Roman, JWST): in no MIME database; application/x-asdf is the name the
    // format's own tools use.
    ("asdf", Some("application/x-asdf")),
    ("root", Some("application/x-root")),
    // SER planetary video (FireCapture, SharpCap): in no MIME database; ours is
    // video/x-ser, with magic, as `.ser` also names FEI TIA microscope files.
    ("ser", Some("video/x-ser")),
    // Scientific arrays. shared-mime-info names HDF5 application/x-hdf (with magic) and
    // NetCDF application/x-netcdf; EMD (an HDF5 convention) has no type there, ours is
    // application/x-emd, a sub-class of HDF5.
    ("h5", Some("application/x-hdf")),
    ("hdf5", Some("application/x-hdf")),
    ("he5", Some("application/x-hdf")),
    // HDF4 shares the type (shared-mime-info's x-hdf has both signatures and the .hdf, .h4
    // and .hdf4 globs); .he2 and .hdfeos name HDF-EOS2 files.
    ("hdf", Some("application/x-hdf")),
    ("h4", Some("application/x-hdf")),
    ("hdf4", Some("application/x-hdf")),
    ("he2", Some("application/x-hdf")),
    ("hdfeos", Some("application/x-hdf")),
    // OMERO image descriptors: our own files, written to the cache while browsing a server;
    // not registered with the desktop.
    ("omero", None),
    // Remote Zarr links: our own files, written to the cache when a store's address is
    // opened; not registered with the desktop.
    ("zarrurl", None),
    ("nc", Some("application/x-netcdf")),
    ("nc4", Some("application/x-netcdf")),
    ("emd", Some("application/x-emd")),
    // NeXus and Imaris are HDF5 conventions with no shared-mime-info type: ours, sub-classes
    // of HDF5. BAG (`.bag`, shared with ROS bags) and Gaussian cube (`.cube`, shared with
    // colour lookup tables) are claimed by content only and get no glob.
    ("nxs", Some("application/x-nexus")),
    ("nx5", Some("application/x-nexus")),
    ("ims", Some("application/x-imaris")),
    // GRIB: freedesktop has no type; application/x-grib is what file(1) and others use.
    ("grib", Some("application/x-grib")),
    ("grb", Some("application/x-grib")),
    ("grib1", Some("application/x-grib")),
    ("grib2", Some("application/x-grib")),
    ("grb1", Some("application/x-grib")),
    ("grb2", Some("application/x-grib")),
    // SRTM elevation tiles: no signature, not in shared-mime-info.
    ("hgt", Some("application/x-srtm-hgt")),
    // Diffraction detector frames: none is in shared-mime-info, the names are ours. SMV and
    // R-AXIS `.img` (disk images' extension), ESRF `.edf` (EEG recordings' extension) and
    // Bruker's numbered frames are claimed by content only and get no glob; compressed EDF
    // is left unregistered with them.
    ("cbf", Some("image/x-cbf")),
    ("cbf.gz", Some("image/x-cbf")),
    ("cbf.bz2", Some("image/x-cbf")),
    ("edf.gz", None),
    ("edf.bz2", None),
    ("sfrm", Some("image/x-bruker-sfrm")),
    ("mccd", Some("image/x-marccd")),
    ("osc", Some("image/x-rigaku-raxis")),
    // DTED elevation tiles: not in shared-mime-info; ours, with the "UHL1" magic.
    // USGS DEM elevation models (ASCII, 1024-byte records): no registered type.
    ("dem", Some("application/x-usgs-dem")),
    ("dt0", Some("application/x-dted")),
    ("dt1", Some("application/x-dted")),
    ("dt2", Some("application/x-dted")),
    // Raw band-interleaved data: readable only beside its `.hdr` header, so not
    // registered. ENVI headers (`.hdr`, Radiance's glob), ASCII grids (`.asc`, PGP's) and
    // ERDAS IMAGINE (`.img`, disk images') are recognised by content without a glob.
    ("bil", None),
    ("bsq", None),
    ("bip", None),
    // More gridded rasters (none in shared-MIME-info: our own names). BSB/KAP nautical charts
    // and their `.bsb` descriptions; ER Mapper headers (the data beside has no extension);
    // SAGA grid headers and their ZIP (`.sdat` data opens through its header, so is not
    // registered); GDAL virtual rasters (an OGR vector `.vrt` is refused). An Arc/Info grid
    // is a folder: its `.adf` files are not registered.
    ("kap", Some("application/x-bsb-chart")),
    ("bsb", Some("application/x-bsb-chart")),
    ("ers", Some("application/x-ermapper")),
    ("sgrd", Some("application/x-saga-grid")),
    ("sg-grd-z", Some("application/x-saga-grid")),
    ("vrt", Some("application/x-gdal-vrt")),
    // Seismic data: neither is in shared-mime-info; application/x-segy is the name other
    // seismic tools use.
    ("sgy", Some("application/x-segy")),
    ("segy", Some("application/x-segy")),
    ("su", Some("application/x-seismic-unix")),
    // Imaging mass spectrometry: not in shared-mime-info; the name others use. The `.ibd`
    // binary beside it is not opened alone.
    ("imzml", Some("application/x-imzml")),
    // Thermal stills: none is in shared-mime-info (R-JPEGs are image/jpeg); the names are
    // ours.
    ("fff", Some("application/x-flir-fff")),
    ("seq", Some("video/x-flir-seq")),
    ("csq", Some("video/x-flir-seq")),
    ("irg", Some("application/x-infiray-irg")),
    ("irb", Some("application/x-infratec-irb")),
    ("bmt", Some("application/x-testo-bmt")),
    ("is2", Some("application/x-fluke-is2")),
    // Reflectance transformation images: neither HP's PTM nor CHI's RTI (HSH) is in
    // shared-mime-info; the names are ours.
    ("ptm", Some("image/x-ptm")),
    ("rti", Some("image/x-rti")),
    ("hsh", Some("image/x-rti")),
    // Microscopy: none of these is in shared-mime-info. Gwyddion itself registers
    // application/x-gwyddion; the others are ours. `.map` (CCP4 / MRC maps) is not claimed:
    // too generic.
    ("dm3", Some("image/x-gatan-dm")),
    ("dm4", Some("image/x-gatan-dm")),
    ("mrc", Some("image/x-mrc")),
    ("mrcs", Some("image/x-mrc")),
    ("ccp4", Some("image/x-mrc")),
    // SPIDER (cryo-EM) and McIDAS AREA (satellite imagery): ours.
    ("spi", Some("image/x-spider")),
    ("spider", Some("image/x-spider")),
    ("area", Some("image/x-mcidas-area")),
    ("ara", Some("image/x-mcidas-area")),
    ("gwy", Some("application/x-gwyddion")),
    ("ibw", Some("application/x-igor-binary-wave")),
    // Arrays and lab data: none of these are in shared-mime-info.
    ("npy", Some("application/x-npy")),
    ("npz", Some("application/x-npz")),
    ("mat", Some("application/x-matlab-data")),
    ("tdms", Some("application/x-tdms")),
    ("vtk", Some("application/x-vtk")),
    ("vti", Some("application/x-vtk-image-data")),
    ("vtr", Some("application/x-vtk-rectilinear-grid")),
    ("vts", Some("application/x-vtk-structured-grid")),
    ("vtu", Some("application/x-vtk-unstructured-grid")),
    ("vtp", Some("application/x-vtk-polydata")),
    // Materials science: none of these is in shared-mime-info; all are ours. FEI TIA series
    // share `.ser` with SER videos (above) and are told apart by content; Nanoscope files
    // are mostly numbered (`.001`) and Zygo MetroPro's `.dat` is everyone's, so only
    // Nanoscope's `.spm` is claimed by name.
    ("emi", Some("application/x-fei-emi")),
    ("bcf", Some("application/x-bruker-bcf")),
    ("ang", Some("application/x-edax-ang")),
    ("ctf", Some("application/x-oxford-ctf")),
    ("spm", Some("application/x-nanoscope")),
    ("sxm", Some("application/x-nanonis-sxm")),
    ("jpk", Some("application/x-jpk")),
    ("mdt", Some("application/x-ntmdt")),
    ("x3p", Some("application/x-x3p")),
    ("datx", Some("application/x-zygo-datx")),
    ("vk4", Some("application/x-keyence-vk4")),
    ("plu", Some("application/x-sensofar-plu")),
    ("al3d", Some("application/x-alicona-al3d")),
    ("sur", Some("application/x-digitalsurf-sur")),
    // Vendor light microscopy: neither is in shared-mime-info; the x- names are ours, with
    // magic (LIF's 0x70 block header, ND2's chunk signature or JPEG 2000 box).
    ("lif", Some("image/x-leica-lif")),
    ("lof", Some("image/x-leica-lif")),
    ("nd2", Some("image/x-nikon-nd2")),
    // Olympus FluoView OIR ("OLYMPUSRAWFORMAT"); Zeiss CZI ("ZISRAWFILE", read by libCZI
    // when it is installed).
    ("oir", Some("image/x-olympus-oir")),
    // ICS: `.ics` is iCalendar's too, so the header is registered by its magic only
    // (CONTENT_MIME); the `.ids` samples have none.
    ("ics", None),
    ("ids", None),
    ("czi", Some("image/x-zeiss-czi")),
    // Spectroscopy cameras. SPE has no signature, but `.spe` is its own; `.sif` is also
    // Singularity's container images: Andor SIF is registered by its magic only
    // (CONTENT_MIME).
    ("spe", Some("image/x-princeton-spe")),
    // Flow cytometry: ISAC's registered type; Beckman Coulter's `.lmd` list-mode files are
    // FCS too.
    ("fcs", Some("application/vnd.isac.fcs")),
    ("lmd", Some("application/vnd.isac.fcs")),
    ("sif", None),
    // Drawings. shared-mime-info registers image/vnd.dxf (with magic) for DXF.
    ("dxf", Some("image/vnd.dxf")),
    // HP-GL plots. shared-mime-info registers application/vnd.hp-hpgl for `.hpgl` only;
    // the package adds the other plot-file globs.
    ("plt", Some("application/vnd.hp-hpgl")),
    ("hpgl", Some("application/vnd.hp-hpgl")),
    ("hpg", Some("application/vnd.hp-hpgl")),
    ("hp2", Some("application/vnd.hp-hpgl")),
    ("hgl", Some("application/vnd.hp-hpgl")),
    ("pen", Some("application/vnd.hp-hpgl")),
    // IC layouts: not in shared-mime-info; application/vnd.gds is the name layout tools use.
    // The package adds it with the HEADER/BGNLIB magic.
    ("gds", Some("application/vnd.gds")),
    ("gds2", Some("application/vnd.gds")),
    ("gdsii", Some("application/vnd.gds")),
    // Circuit boards. Gerber has the IANA type application/vnd.gerber, which shared-mime-info
    // lacks; the package declares it with its format statement and attribute magic. Eagle's
    // .cmp, .sol, .stc, .sts, .plc, .pls, .crc and .crs (and .exc) name other things too, so
    // they are read but not registered. Excellon and KiCad have no IANA types; the names are
    // gerbv's and KiCad's own. Eagle's .gml outline is GML's extension: such files are
    // recognised by content.
    ("gbr", Some("application/vnd.gerber")),
    ("ger", Some("application/vnd.gerber")),
    ("gbx", Some("application/vnd.gerber")),
    ("pho", Some("application/vnd.gerber")),
    ("art", Some("application/vnd.gerber")),
    ("gtl", Some("application/vnd.gerber")),
    ("gbl", Some("application/vnd.gerber")),
    ("gto", Some("application/vnd.gerber")),
    ("gbo", Some("application/vnd.gerber")),
    ("gts", Some("application/vnd.gerber")),
    ("gbs", Some("application/vnd.gerber")),
    ("gtp", Some("application/vnd.gerber")),
    ("gbp", Some("application/vnd.gerber")),
    ("gko", Some("application/vnd.gerber")),
    ("gm1", Some("application/vnd.gerber")),
    ("gm2", Some("application/vnd.gerber")),
    ("gm3", Some("application/vnd.gerber")),
    ("gm4", Some("application/vnd.gerber")),
    ("gm13", Some("application/vnd.gerber")),
    ("gm15", Some("application/vnd.gerber")),
    ("gp1", Some("application/vnd.gerber")),
    ("gp2", Some("application/vnd.gerber")),
    ("g1", Some("application/vnd.gerber")),
    ("g2", Some("application/vnd.gerber")),
    ("g3", Some("application/vnd.gerber")),
    ("g4", Some("application/vnd.gerber")),
    ("g5", Some("application/vnd.gerber")),
    ("g6", Some("application/vnd.gerber")),
    ("gd1", Some("application/vnd.gerber")),
    ("gg1", Some("application/vnd.gerber")),
    ("cmp", None),
    ("sol", None),
    ("stc", None),
    ("sts", None),
    ("plc", None),
    ("pls", None),
    ("crc", None),
    ("crs", None),
    ("drl", Some("application/x-excellon")),
    ("xln", Some("application/x-excellon")),
    ("drd", Some("application/x-excellon")),
    ("exc", None),
    ("kicad_pcb", Some("application/x-kicad-pcb")),
    ("kicad_sch", Some("application/x-kicad-schematic")),
    // IPC-2581 (Cadence's `.cvg`; other tools write `.xml`, recognised by content). No
    // registered type; the package declares it with the root-element magic. ODB++ jobs are
    // folders or archives, recognised by their paths, not by a name of their own.
    ("cvg", Some("application/x-ipc2581")),
    // Spectra. JCAMP-DX and SPC have the chemical-mime-data names; .spc is also a PKCS #7
    // certificate file in shared-mime-info, so it is read but not registered. mzML and mzXML
    // have no registered types; the names are the ones other tools use.
    ("jdx", Some("chemical/x-jcamp-dx")),
    ("dx", Some("chemical/x-jcamp-dx")),
    ("jcm", Some("chemical/x-jcamp-dx")),
    ("jcamp", Some("chemical/x-jcamp-dx")),
    ("spc", None),
    ("mzml", Some("application/x-mzml")),
    ("mzxml", Some("application/x-mzxml")),
    // Whole-file gzip, as PRIDE and MassIVE serve them: the type by name, as for FITS.
    ("mzml.gz", Some("application/x-mzml")),
    ("mzxml.gz", Some("application/x-mzxml")),
    // 3D meshes. shared-mime-info has model/stl, model/obj, model/gltf+json,
    // model/gltf-binary and model/3mf; PLY has no type there, so the package declares
    // model/x-ply (the name MeshLab and others use) with its "ply" magic.
    ("ply", Some("model/x-ply")),
    // Geomview OFF meshes: no registered type.
    ("off", Some("model/x-off")),
    ("glb", Some("model/gltf-binary")),
    ("stl", Some("model/stl")),
    ("obj", Some("model/obj")),
    ("gltf", Some("model/gltf+json")),
    ("3mf", Some("model/3mf")),
    // Alembic (not in shared-MIME-info; the name its tools use).
    ("abc", Some("model/x-alembic")),
    // Point clouds. LAS, LAZ (and COPC, `.copc.laz`) and E57 have IANA types
    // (application/vnd.las, application/vnd.laszip, model/e57) that shared-mime-info lacks;
    // the package declares them with their magic. shared-mime-info gives `.pcd` to Kodak
    // Photo CD, so PCD gets its own x- type, told apart by its header; XYZ and PTS have none.
    ("las", Some("application/vnd.las")),
    ("laz", Some("application/vnd.laszip")),
    ("e57", Some("model/e57")),
    ("pcd", Some("model/x-pcd")),
    ("xyz", Some("model/x-xyz")),
    ("pts", Some("model/x-pts")),
    ("ptx", Some("model/x-ptx")),
    // Gaussian splats: none has a registered type; the package declares x- types (version 4
    // `.spz` with its "NGSP" magic). `.ksplat` is recognised only to be refused. Splat PLY
    // files are model/x-ply.
    ("splat", Some("model/x-gaussian-splat")),
    ("spz", Some("model/x-spz")),
    ("ksplat", None),
    // Scene formats. shared-mime-info has none of USD, COLLADA, 3DS, LightWave or FBX; the
    // package declares the IANA names where they exist (model/vnd.usda, model/vnd.usdz+zip,
    // model/vnd.collada+xml) and x- names otherwise. Binary USD shares model/x-usd with `.usd`.
    ("usd", Some("model/x-usd")),
    ("usda", Some("model/vnd.usda")),
    ("usdc", Some("model/x-usd")),
    ("usdz", Some("model/vnd.usdz+zip")),
    ("dae", Some("model/vnd.collada+xml")),
    ("zae", Some("model/x-collada-zae")),
    ("3ds", Some("model/x-3ds")),
    ("lwo", Some("model/x-lwo")),
    ("fbx", Some("model/x-fbx")),
    // X3D has IANA types (model/x3d+xml, model/x3d-vrml) that shared-mime-info lacks; the
    // package declares them, gzipped files under the same types. VRML 97 is shared-mime-info's
    // model/vrml (by "#VRML"). Rhino and AMF have none: x- types.
    ("x3d", Some("model/x3d+xml")),
    ("x3dz", Some("model/x3d+xml")),
    ("x3dv", Some("model/x3d-vrml")),
    ("x3dvz", Some("model/x3d-vrml")),
    ("wrl", Some("model/vrml")),
    ("wrz", Some("model/vrml")),
    ("vrml", Some("model/vrml")),
    ("3dm", Some("model/x-3dm")),
    ("amf", Some("model/x-amf")),
    // Previews only: shared-mime-info has application/x-blender; SketchUp's is the name
    // SketchUp registers.
    ("blend", Some("application/x-blender")),
    ("skp", Some("application/vnd.sketchup.skp")),
    // STEP and IGES, read by OpenCASCADE when it is installed: the IANA names model/step and
    // model/iges (shared-mime-info has model/iges; the package declares model/step, by name
    // only, since IFC starts the same way). IFC has no registered type: the package declares
    // model/x-ifc by name (and model/x-ifczip for packages).
    ("ifc", Some("model/x-ifc")),
    ("ifczip", Some("model/x-ifczip")),
    // ifcXML is recognised to be refused with the reason.
    ("ifcxml", None),
    ("stp", Some("model/step")),
    ("step", Some("model/step")),
    ("igs", Some("model/iges")),
    ("iges", Some("model/iges")),
    // DWG (shared-mime-info: image/vnd.dwg), only through the ODA File Converter.
    ("dwg", Some("image/vnd.dwg")),
    // Molecular structures: the chemical/x- names of chemical-mime-data (Open Babel, Jmol,
    // Avogadro), which shared-mime-info lacks; the package declares them. `.cif` is the
    // small-molecule type (mmCIF is told apart by content); molecular XYZ shares `.xyz`
    // with point clouds (chemical/x-xyz, see SHARED_EXTENSION_MIME) and opens by content. Compressed files carry the
    // type of what is inside, as `.nii.gz` does.
    ("pdb", Some("chemical/x-pdb")),
    ("ent", Some("chemical/x-pdb")),
    ("pdb.gz", Some("chemical/x-pdb")),
    ("ent.gz", Some("chemical/x-pdb")),
    ("mmcif", Some("chemical/x-mmcif")),
    ("mcif", Some("chemical/x-mmcif")),
    ("mmcif.gz", Some("chemical/x-mmcif")),
    ("cif", Some("chemical/x-cif")),
    ("cif.gz", Some("chemical/x-cif")),
    ("sdf", Some("chemical/x-mdl-sdfile")),
    ("sd", Some("chemical/x-mdl-sdfile")),
    ("sdf.gz", Some("chemical/x-mdl-sdfile")),
    ("mol", Some("chemical/x-mdl-molfile")),
    ("mol.gz", Some("chemical/x-mdl-molfile")),
    // Medical research volumes: none are in shared-mime-info. The x- names are the ones
    // 3D Slicer, ITK-SNAP and MRIcroGL users meet; Analyze and NIfTI `.hdr` / `.img` pairs
    // are recognised by content (`.hdr` stays Radiance's).
    ("nii", Some("application/x-nifti")),
    ("nii.gz", Some("application/x-nifti")),
    // MRtrix images: `.mif` is Adobe FrameMaker's in shared-mime-info (recognised by
    // content here); the gzipped and header-only forms are free.
    ("mif", None),
    ("mif.gz", Some("application/x-mrtrix-image")),
    ("mih", Some("application/x-mrtrix-image")),
    // HCP surfaces and maps: none is in shared-mime-info. CIFTI-2 files are NIfTI-2 named
    // by what they hold; the longer glob wins over `*.nii`.
    ("gii", Some("application/x-gifti")),
    ("dscalar.nii", Some("application/x-cifti")),
    ("dlabel.nii", Some("application/x-cifti")),
    ("dtseries.nii", Some("application/x-cifti")),
    ("pscalar.nii", Some("application/x-cifti")),
    ("plabel.nii", Some("application/x-cifti")),
    ("ptseries.nii", Some("application/x-cifti")),
    ("dconn.nii", Some("application/x-cifti")),
    // Tractograms: not in shared-mime-info.
    ("trk", Some("application/x-trackvis")),
    ("tck", Some("application/x-mrtrix-tracks")),
    // Philips PAR/REC: not in shared-mime-info. The header takes the type; `.rec` is too
    // generic to claim (a REC beside its PAR is left out of folders and opens by name).
    // ECAT 7 (`.v`, Verilog's too) and DeltaVision (`.dv`, DV video's) are recognised by
    // content when opened; they get no type of their own.
    ("par", Some("application/x-philips-parrec")),
    ("rec", None),
    // FreeSurfer: not in shared-mime-info.
    ("mgh", Some("application/x-freesurfer-mgh")),
    ("mgz", Some("application/x-freesurfer-mgh")),
    // MINC (HDF5 or NetCDF inside): not in shared-mime-info.
    ("mnc", Some("application/x-minc")),
    ("nrrd", Some("application/x-nrrd")),
    ("nhdr", Some("application/x-nrrd")),
    ("mha", Some("application/x-metaimage")),
    ("mhd", Some("application/x-metaimage")),
    // Industrial CT: none is in shared-mime-info. Zeiss Xradia files are OLE compound files
    // (a sub-class of application/x-ole-storage); Amira has the "# AmiraMesh" / "# Avizo"
    // line; Nikon X-Tek and VGStudio headers are INI text naming the volume beside them. A
    // SkyScan reconstruction opens from its `.log`, recognised by content (`.log` is not
    // claimed). Headerless `.raw` / `.vol` samples are too generic to register.
    ("txm", Some("application/x-xradia-txm")),
    ("txrm", Some("application/x-xradia-txm")),
    ("xrm", Some("application/x-xradia-txm")),
    ("am", Some("application/x-amira-mesh")),
    ("xtekct", Some("application/x-xtek-ct")),
    ("xtekvolume", Some("application/x-xtek-ct")),
    ("vgi", Some("application/x-vgstudio-vgi")),
    ("raw", None),
    ("vol", None),
    // Whole-slide images and microscopy pyramids
    ("svs", Some("image/x-aperio-svs")),
    ("ndpi", Some("image/x-hamamatsu-ndpi")),
    ("scn", Some("image/x-leica-scn")),
    ("ome.tif", Some("image/x-ome-tiff")),
    ("ome.tiff", Some("image/x-ome-tiff")),
    ("ome.btf", Some("image/x-ome-tiff")),
    // MIRAX: the `.mrxs` beside its folder. Ventana `.bif` is also Roku's trick-play index
    // and Olympus `.vsi` Visual Studio's content installer: BIF is not registered (a TIFF
    // with nothing to tell it by), VSI by its metadata header only (CONTENT_MIME).
    ("mrxs", Some("image/x-mirax")),
    // Hamamatsu VMS / VMU index files.
    ("vms", Some("image/x-hamamatsu-vms")),
    ("vmu", Some("image/x-hamamatsu-vmu")),
    // Philips iSyntax is recognised only to be refused: not offered.
    ("isyntax", None),
    ("bif", None),
    ("vsi", None),
    // OME-Zarr stores are folders (or zips of one): no file type of their own.
    ("ome.zarr", None),
    ("zarr", None),
    ("zarr.zip", None),
    // Geodata. shared-mime-info has GeoJSON, KML, KMZ and GPX; Shapefiles and GeoPackages are
    // not in it, so the package declares application/x-shapefile (the main `.shp` only; its
    // sidecars are not opened alone) and the IANA application/geopackage+sqlite3 (magic: the
    // SQLite application id "GPKG" at byte 68). A GeoTIFF is told apart by its GeoKey
    // directory, not its name: Specular reports it as `image/tiff; application=geotiff`
    // (Format::GeoTiff), and the desktop database keeps image/tiff for `.tif`.
    ("geojson", Some("application/geo+json")),
    ("shp", Some("application/x-shapefile")),
    ("gpkg", Some("application/geopackage+sqlite3")),
    ("kml", Some("application/vnd.google-earth.kml+xml")),
    ("kmz", Some("application/vnd.google-earth.kmz")),
    ("gpx", Some("application/gpx+xml")),
    // Web map tile sets: neither is in shared-mime-info. MBTiles is SQLite with the
    // application id "MPBX" at byte 68; PMTiles starts with "PMTiles" and its version byte.
    ("mbtiles", Some("application/x-mbtiles")),
    ("pmtiles", Some("application/vnd.pmtiles")),
    // More vector formats. GML is in shared-mime-info; Parquet and Arrow have IANA types
    // (only files with a geometry column open); OSM XML and TCX have IANA types; the others
    // take the names their projects use. A PBF is only claimed as `.osm.pbf` (`.pbf` alone
    // is any protobuf, map tiles included). A File Geodatabase is a folder: its tables are
    // not registered. `.mif` stays FrameMaker's (above) and `.fit` FITS's, with Garmin FIT
    // as the second type (SHARED_EXTENSION_MIME); both open by content.
    ("fgb", Some("application/flatgeobuf")),
    ("parquet", Some("application/vnd.apache.parquet")),
    ("geoparquet", Some("application/vnd.apache.parquet")),
    ("arrow", Some("application/vnd.apache.arrow.file")),
    ("feather", Some("application/vnd.apache.arrow.file")),
    ("arrows", Some("application/vnd.apache.arrow.stream")),
    ("gml", Some("application/gml+xml")),
    ("osm", Some("application/vnd.openstreetmap.data+xml")),
    ("osm.pbf", Some("application/x-osm-pbf")),
    ("tcx", Some("application/vnd.garmin.tcx+xml")),
    ("tab", Some("application/x-mapinfo-tab")),
    ("gdb", None),
    ("gdbtable", None),
    ("gdbtablx", None),
    // S-57 chart cells, IGC flight logs, NMEA logs and TopoJSON: none is in shared-MIME-info
    // (our own names). `.000` is also split archives and backups: told apart by its ISO 8211
    // header, not registered.
    ("000", None),
    ("topojson", Some("application/x-topojson")),
    ("igc", Some("application/x-igc")),
    ("nmea", Some("application/x-nmea")),
    // NITF imagery (IANA application/vnd.nitf). ECW and MrSID are refused (their SDKs' licences
    // allow no other program), so not registered.
    ("ntf", Some("application/vnd.nitf")),
    ("nitf", Some("application/vnd.nitf")),
    ("nsf", Some("application/vnd.nitf")),
    ("ecw", None),
    ("sid", None),
    // Movies (opened through specular-video). Offered for and thumbnailed, never claimed by
    // the default-viewer option (it takes image/ types only).
    ("mp4", Some("video/mp4")),
    // Camera files that are MP4: GoPro proxies and 360° clips, Insta360 clips.
    ("lrv", Some("video/mp4")),
    ("360", Some("video/mp4")),
    ("insv", Some("video/mp4")),
    ("m4v", Some("video/x-m4v")),
    // Adobe's Flash MP4: also video/mp4 (SHARED_EXTENSION_MIME).
    ("f4v", Some("video/x-f4v")),
    ("mov", Some("video/quicktime")),
    ("qt", Some("video/quicktime")),
    ("3gp", Some("video/3gpp")),
    ("3g2", Some("video/3gpp2")),
    ("mkv", Some("video/x-matroska")),
    ("mk3d", Some("video/x-matroska")),
    // Matroska sound: shown as a sound file.
    ("mka", Some("audio/x-matroska")),
    ("webm", Some("video/webm")),
    // Transport streams; AVCHD and Blu-ray files are also video/vnd.dlna.mpeg-tts
    // (SHARED_EXTENSION_MIME). `.ts` is also TypeScript: opened by content only.
    ("mts", Some("video/mp2t")),
    ("m2ts", Some("video/mp2t")),
    ("ts", Some("video/mp2t")),
    ("m2t", Some("video/mp2t")),
    // JVC camcorders' transport streams.
    ("tod", Some("video/mp2t")),
    // Broadcast captures. `.tp` is also other programs' (opened by content).
    ("trp", Some("video/mp2t")),
    ("tp", Some("video/mp2t")),
    // MPEG programme streams and bare MPEG video. DVD `.vob` is also video/mpeg
    // (SHARED_EXTENSION_MIME). Camcorder `.mod` is also a tracker module: opened by content
    // only.
    ("mpg", Some("video/mpeg")),
    ("mpeg", Some("video/mpeg")),
    ("mpe", Some("video/mpeg")),
    ("m2p", Some("video/mpeg")),
    ("vro", Some("video/mpeg")),
    ("m1v", Some("video/mpeg")),
    ("m2v", Some("video/mpeg")),
    ("mpv", Some("video/mpeg")),
    ("vob", Some("video/x-ms-vob")),
    // `.mod`: a tracker module (told by its tag at byte 1080) or a camcorder's MPEG
    // programme stream (told by its pack header); registered as the module.
    ("mod", Some("audio/x-mod")),
    // HD DVD video objects (unencrypted only).
    ("evo", Some("video/mpeg")),
    ("evob", Some("video/mpeg")),
    // AVI (DivX, Xvid, camera clips).
    ("avi", Some("video/x-msvideo")),
    ("divx", Some("video/x-msvideo")),
    // Ogg movies (Theora; OGM's DivX/Xvid). `.ogg` is usually sound only: opened by content only.
    ("ogv", Some("video/ogg")),
    ("ogm", Some("video/x-ogm+ogg")),
    ("ogg", None),
    // Raw VP8, VP9 and AV1 streams.
    ("ivf", Some("video/x-ivf")),
    // Raw H.264, HEVC and AV1 streams: not in shared-MIME-info; ours.
    ("h264", Some("video/x-h264")),
    ("264", Some("video/x-h264")),
    ("avc", Some("video/x-h264")),
    ("h265", Some("video/x-h265")),
    ("265", Some("video/x-h265")),
    ("hevc", Some("video/x-h265")),
    ("obu", Some("video/x-av1")),
    // Raw APV files: not in shared-MIME-info (the draft registers no type); ours.
    ("apv", Some("video/x-apv")),
    // QOA sound: not in shared-MIME-info; ours.
    ("qoa", Some("audio/x-qoa")),
    // Motion JPEG streams: not in shared-MIME-info; ours.
    ("mjpeg", Some("video/x-motion-jpeg")),
    ("mjpg", Some("video/x-motion-jpeg")),
    // MXF (broadcast, post-production and camera files).
    ("mxf", Some("application/mxf")),
    // Flash Video, Windows Media (ASF) and raw DV.
    ("flv", Some("video/x-flv")),
    ("wmv", Some("video/x-ms-wmv")),
    ("asf", Some("application/vnd.ms-asf")),
    // Media Center recordings of Windows XP and Vista (ASF); protected ones are refused.
    ("dvr-ms", Some("video/x-ms-dvr")),
    // Their successor (Windows Vista TV Pack to 8.1); protected ones are refused too.
    ("wtv", Some("video/wtv")),
    ("dv", Some("video/dv")),
    ("dif", Some("video/dv")),
    // Uncompressed YUV4MPEG2 streams and CD+G karaoke graphics.
    ("y4m", Some("video/x-yuv4mpeg")),
    ("cdg", Some("video/x-cdg")),
    // Sound files (shown as their cover and waveform, with play controls). `.ogg` is under
    // Ogg movies above (read by content).
    ("mp3", Some("audio/mpeg")),
    ("mpga", Some("audio/mpeg")),
    ("mp2", Some("audio/mp2")),
    ("mp1", Some("audio/mpeg")),
    ("mpa", Some("audio/mpeg")),
    ("m1a", Some("audio/mpeg")),
    ("m2a", Some("audio/mpeg")),
    ("aac", Some("audio/aac")),
    ("adts", Some("audio/aac")),
    ("flac", Some("audio/flac")),
    ("wav", Some("audio/x-wav")),
    ("rf64", Some("audio/x-rf64")),
    ("bw64", Some("audio/x-bw64")),
    ("au", Some("audio/basic")),
    ("snd", Some("audio/basic")),
    ("caf", Some("audio/x-caf")),
    ("w64", Some("audio/x-w64")),
    ("voc", Some("audio/x-voc")),
    ("8svx", Some("audio/x-8svx")),
    ("16sv", Some("audio/x-8svx")),
    ("svx", Some("audio/x-8svx")),
    ("avr", Some("audio/x-avr")),
    ("sph", Some("audio/x-nist-sphere")),
    ("nist", Some("audio/x-nist-sphere")),
    ("sf", Some("audio/x-ircam")),
    ("ircam", Some("audio/x-ircam")),
    ("tta", Some("audio/x-tta")),
    // Shared-MIME-info's name for Shorten.
    ("shn", Some("application/x-shorten")),
    ("dsf", Some("audio/x-dsf")),
    ("dff", Some("audio/x-dff")),
    ("s3m", Some("audio/x-s3m")),
    ("xm", Some("audio/x-xm")),
    ("it", Some("audio/x-it")),
    ("aiff", Some("audio/x-aiff")),
    ("aif", Some("audio/x-aiff")),
    ("aifc", Some("audio/x-aifc")),
    ("dts", Some("audio/vnd.dts")),
    ("ac3", Some("audio/ac3")),
    ("eac3", Some("audio/eac3")),
    ("amr", Some("audio/AMR")),
    ("awb", Some("audio/AMR-WB")),
    ("wv", Some("audio/x-wavpack")),
    ("ape", Some("audio/x-ape")),
    ("mpc", Some("audio/x-musepack")),
    ("mp+", Some("audio/x-musepack")),
    ("mpp", Some("audio/x-musepack")),
    ("oga", Some("audio/ogg")),
    ("opus", Some("audio/x-opus+ogg")),
    ("spx", Some("audio/x-speex")),
    ("m4a", Some("audio/mp4")),
    ("m4b", Some("audio/x-m4b")),
    // Speech: GSM 06.10 (raw frames), Asterisk's signed linear (headerless, by extension;
    // `.raw` is not claimed), HTK recordings.
    ("gsm", Some("audio/x-gsm")),
    // QCELP and EVRC speech from CDMA phones (RFC 3625).
    ("qcp", Some("audio/qcelp")),
    // Plain `.sln` is also a Visual Studio solution: opened in-app, never registered.
    ("sln", None),
    ("sln12", Some("audio/x-sln")),
    ("sln16", Some("audio/x-sln")),
    ("sln24", Some("audio/x-sln")),
    ("sln32", Some("audio/x-sln")),
    ("sln44", Some("audio/x-sln")),
    ("sln48", Some("audio/x-sln")),
    ("sln96", Some("audio/x-sln")),
    ("sln192", Some("audio/x-sln")),
    ("htk", Some("audio/x-htk")),
    // Instruments and sample banks (their samples listed and played) and MIDI songs
    // (played through the user's SoundFont, else the built-in sounds). `.rmi` is RIFF MIDI.
    ("xi", Some("audio/x-xi")),
    ("sf2", Some("audio/x-soundfont")),
    ("sfz", Some("audio/x-sfz")),
    ("dls", Some("audio/dls")),
    ("mid", Some("audio/midi")),
    ("midi", Some("audio/midi")),
    ("kar", Some("audio/midi")),
    ("rmi", Some("audio/mid")),
    // Playlists (opened as the play queue), by shared-MIME-info's names. A local `.m3u8` is
    // not an HLS stream (application/vnd.apple.mpegurl). `.pls` is above (Gerber's paste
    // layer too): a PLS playlist is told by its `[playlist]` line.
    ("m3u", Some("audio/x-mpegurl")),
    ("m3u8", Some("audio/x-mpegurl")),
    ("xspf", Some("application/xspf+xml")),
    // Text, tables and Markdown (shared-MIME-info's names; `.diz` has no type of its own and
    // is DOS art like `.nfo`). A Bruker SkyScan `.log` is told by its sections.
    ("rtf", Some("application/rtf")),
    ("csv", Some("text/csv")),
    ("tsv", Some("text/tab-separated-values")),
    ("md", Some("text/markdown")),
    ("markdown", Some("text/markdown")),
    ("mdown", Some("text/markdown")),
    ("mkd", Some("text/markdown")),
    ("nfo", Some("text/x-nfo")),
    ("diz", Some("text/x-nfo")),
    ("txt", Some("text/plain")),
    ("text", Some("text/plain")),
    ("log", Some("text/x-log")),
    // Subtitles (shared-MIME-info's names where it has them). PGS, LRC and SCC have none:
    // `x-` names. A VobSub `.idx` is any index file by name: read by content only, not
    // registered (its `.sub` is MicroDVD's glob, told apart by content).
    ("srt", Some("application/x-subrip")),
    ("vtt", Some("text/vtt")),
    ("webvtt", Some("text/vtt")),
    ("ass", Some("text/x-ssa")),
    ("ssa", Some("text/x-ssa")),
    ("sub", Some("text/x-microdvd")),
    ("idx", None),
    ("sup", Some("application/x-pgs")),
    ("lrc", Some("text/x-lrc")),
    ("ttml", Some("application/ttml+xml")),
    ("dfxp", Some("application/ttml+xml")),
    ("scc", Some("text/x-scc")),
    // fmt500e
    ("str", Some("video/x-psx-str")),
    ("669", Some("audio/x-mod")),
    ("mtm", Some("audio/x-mod")),
    ("stm", Some("audio/x-mod")),
    ("okt", Some("audio/x-mod")),
    ("okta", Some("audio/x-mod")),
    ("med", Some("audio/x-mod")),
    ("mmd0", Some("audio/x-mod")),
    ("mmd1", Some("audio/x-mod")),
    ("mmd2", Some("audio/x-mod")),
    ("mmd3", Some("audio/x-mod")),
    ("ult", Some("audio/x-mod")),
    ("far", Some("audio/x-mod")),
    ("pcf", Some("application/x-font-pcf")),
    ("bdf", Some("application/x-font-bdf")),
    ("fon", Some("application/x-font-winfon")),
    ("fnt", Some("application/x-font-winfon")),
    ("psf", Some("application/x-font-linux-psf")),
    ("psfu", Some("application/x-font-linux-psf")),
    ("ipynb", Some("application/x-ipynb+json")),
    ("drawio", Some("application/vnd.jgraph.mxfile")),
    ("dio", Some("application/vnd.jgraph.mxfile")),
    ("excalidraw", Some("application/vnd.excalidraw+json")),
    ("lottie", Some("application/zip+dotlottie")),
    ("swf", Some("application/vnd.adobe.flash.movie")),
    ("md2", Some("model/x-md2")),
    ("ms3d", Some("model/x-ms3d")),
    ("vox", Some("model/x-magicavoxel")),
    ("ac", Some("model/x-ac3d")),
    ("ac3d", Some("model/x-ac3d")),
    ("x", Some("model/x-directx")),
    ("bvh", Some("model/x-bvh")),
    ("mol2", Some("chemical/x-mol2")),
    ("ml2", Some("chemical/x-mol2")),
    ("gro", Some("chemical/x-gromacs-gro")),
];

/// Second types for extensions that two kinds of file share, both read by content: the
/// system database (chemical-mime-data) may name either, so both are registered.
/// `.xyz` is a point cloud (`model/x-xyz`, above) or a molecule (`chemical/x-xyz`).
pub const SHARED_EXTENSION_MIME: &[(&str, &str)] = &[
    // GIMP brushes (shared-MIME-info's type), told apart from Gerber by "GIMP" at byte 20.
    ("gbr", "image/x-gimp-gbr"),
    // Aseprite's short extension, told apart from Swatch Exchange by its magic.
    ("ase", "image/x-aseprite"),
    ("xyz", "chemical/x-xyz"),
    // Kodak Photo CD (shared-MIME-info's type for `.pcd`), told apart by "PCD_IPI".
    ("pcd", "image/x-photo-cd"),
    ("fit", "application/vnd.ant.fit"),
    ("mts", "video/vnd.dlna.mpeg-tts"),
    ("m2ts", "video/vnd.dlna.mpeg-tts"),
    ("vob", "video/mpeg"),
    ("f4v", "video/mp4"),
];

/// Types read by content only, with no glob of their own because their extension is another
/// format's: ESRF EDF (`.edf`, EEG recordings' extension), SMV diffraction frames (`.img`,
/// disk images') and Gaussian cube volumes (`.cube`, colour lookup tables'). The package
/// declares the first two with their magic and the cube under chemical-mime-data's name
/// (which carries the glob there), so the desktop offers Specular for them. CCP4 maps are
/// image/x-mrc by its `MAP ` magic. LandXML survey files are `.xml`: neither IANA nor
/// shared-mime-info has a type for them, so the package declares `application/x-landxml+xml`
/// by its root element. X cursors have no extension; Softimage PIC shares `.pic` with
/// Radiance and GEM bit images `.img` with disk images: all three by magic. Esri ASCII grids
/// share `.asc` with PGP armour: by their `ncols` header.
pub const CONTENT_MIME: &[&str] = &[
    "image/x-esrf-edf",
    "image/x-smv",
    "chemical/x-gaussian-cube",
    "image/x-olympus-vsi",
    "image/x-andor-sif",
    "image/x-ics",
    "application/x-landxml+xml",
    "image/x-xcursor",
    "image/x-softimage-pic",
    "image/x-gem",
    "application/x-esri-ascii-grid",
];

/// The MIME type registered for a (lower-case) extension: `None` when the extension is not
/// in the table, `Some(None)` when it is deliberately unregistered.
pub fn mime_of_extension(ext: &str) -> Option<Option<&'static str>> {
    EXTENSION_MIME
        .iter()
        .find(|(e, _)| *e == ext)
        .map(|(_, m)| *m)
}

impl super::Registry {
    /// Every extension of every decoder, in registry order (no duplicates).
    pub fn extensions(&self) -> Vec<&'static str> {
        let mut out: Vec<&'static str> = Vec::new();
        for d in &self.decoders {
            for e in d.extensions() {
                if !out.contains(e) {
                    out.push(e);
                }
            }
        }
        out
    }

    /// (MIME type, extensions) for every registered extension, grouped by type in registry
    /// order, then the [`CONTENT_MIME`] types without extensions. Extensions missing from
    /// [`EXTENSION_MIME`] or marked `None` are left out.
    pub fn mime_groups(&self) -> Vec<(&'static str, Vec<&'static str>)> {
        let mut out: Vec<(&'static str, Vec<&'static str>)> = Vec::new();
        for ext in self.extensions() {
            let Some(Some(mime)) = mime_of_extension(ext) else {
                continue;
            };
            let shared = SHARED_EXTENSION_MIME
                .iter()
                .filter(|(e, _)| *e == ext)
                .map(|(_, m)| *m);
            for mime in std::iter::once(mime).chain(shared) {
                match out.iter_mut().find(|(m, _)| *m == mime) {
                    Some((_, exts)) => exts.push(ext),
                    None => out.push((mime, vec![ext])),
                }
            }
        }
        out.extend(CONTENT_MIME.iter().map(|m| (*m, Vec::new())));
        out
    }
}

/// The kinds of file the Open dialog offers one by one, after "All supported files", in the
/// order it lists them. Every extension the decoders claim falls in exactly one
/// ([`Registry::file_groups`](super::Registry::file_groups)).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FileGroup {
    Images,
    CameraRaw,
    Video,
    Sound,
    Documents,
    Drawings,
    Models,
    Maps,
    Science,
    Fonts,
    Other,
}

impl FileGroup {
    /// Every group, in the dialog's order.
    pub const ALL: [FileGroup; 11] = [
        FileGroup::Images,
        FileGroup::CameraRaw,
        FileGroup::Video,
        FileGroup::Sound,
        FileGroup::Documents,
        FileGroup::Drawings,
        FileGroup::Models,
        FileGroup::Maps,
        FileGroup::Science,
        FileGroup::Fonts,
        FileGroup::Other,
    ];
}

/// Parts of an `image/` subtype that mark a camera's raw file.
const RAW_HINTS: &[&str] = &[
    "x-adobe-dng",
    "x-canon-",
    "x-epson-erf",
    "x-fuji-raf",
    "x-gopro-gpr",
    "x-hasselblad",
    "x-kodak-",
    "x-leaf-mos",
    "x-leica-rwl",
    "x-mamiya",
    "x-minolta",
    "x-nikon-nef",
    "x-nikon-nrw",
    "x-olympus-orf",
    "x-panasonic",
    "x-pentax",
    "x-phaseone",
    "x-samsung-srw",
    "x-sigma",
    "x-sony-arw",
    "x-sony-sr",
    "x-arri-ari",
    "x-sinar",
    "x-quicktake",
    "x-ricoh-j6i",
    "x-lytro",
];

/// Parts of an `image/` subtype that mark a scientific picture: telescopes, microscopes,
/// slide scanners, detectors.
const SCIENCE_IMAGE_HINTS: &[&str] = &[
    "fits",
    "fpack",
    "x-mrc",
    "x-spider",
    "x-mcidas",
    "x-cbf",
    "x-gatan",
    "x-zeiss",
    "x-leica-lif",
    "x-leica-scn",
    "x-ome-",
    "x-aperio",
    "x-hamamatsu",
    "x-mirax",
    "x-nikon-nd2",
    "x-olympus-oir",
    "x-olympus-vsi",
    "x-metamorph",
    "x-marccd",
    "x-bruker",
    "x-princeton",
    "x-rigaku",
    "x-xisf",
    "x-vicar",
    "x-ptm",
    "x-rti",
    "x-esrf",
    "x-smv",
    "x-andor",
    "x-ics",
];

/// Parts of a subtype that mark a drawing: vector art, diagrams, CAD and circuit boards.
const DRAWING_HINTS: &[&str] = &[
    "svg",
    "cgm",
    "emf",
    "wmf",
    "x-emz",
    "x-wmz",
    "dwg",
    "dxf",
    "x-eps",
    "illustrator",
    "corel-draw",
    "visio",
    "x-wpg",
    "hpgl",
    "gerber",
    "excellon",
    "kicad",
    "ipc2581",
    "vnd.gds",
    "opendocument.graphics",
    "jgraph",
    "excalidraw",
    "dotlottie",
    "flash.movie",
];

/// Parts of an `application/` subtype that mark a document: pages, books, office files.
const DOCUMENT_HINTS: &[&str] = &[
    "pdf",
    "ipynb",
    "x-dvi",
    "epub",
    "msword",
    "ms-excel",
    "ms-powerpoint",
    "opendocument.text",
    "apple.keynote",
    "apple.numbers",
    "apple.pages",
    "xps",
    "ebook",
    "mobi",
    "fictionbook",
    "fb2",
];

/// Parts of an `application/` subtype that mark a map, a track or terrain.
const MAP_HINTS: &[&str] = &[
    "geo+json",
    "geopackage",
    "gml+xml",
    "gpx",
    "flatgeobuf",
    "kml",
    "kmz",
    "openstreetmap",
    "osm-pbf",
    "shapefile",
    "mapinfo",
    "tcx",
    "dted",
    "usgs-dem",
    "esri-ascii-grid",
    "srtm",
    "pmtiles",
    "mbtiles",
    "ant.fit",
    "nitf",
    "apache.parquet",
    "apache.arrow",
    "bsb-chart",
    "ermapper",
    "saga-grid",
    "gdal-vrt",
    "topojson",
    "x-igc",
    "x-nmea",
];

/// Parts of an `application/` subtype that mark a layered picture of an editing program.
const LAYERED_IMAGE_HINTS: &[&str] = &[
    "affinity",
    "krita",
    "procreate",
    "clip-studio",
    "x-sketch",
    "figma",
    "navi-animation",
];

/// Parts of an `application/` subtype that mark a 3D scene or a scan of points.
const MODEL_HINTS: &[&str] = &["blender", "sketchup", "vnd.las"];

/// Parts of a subtype that mark subtitles and lyrics (grouped with video).
const SUBTITLE_HINTS: &[&str] = &[
    "subrip", "vtt", "x-ssa", "microdvd", "ttml", "x-pgs", "x-lrc", "x-scc",
];

/// The Open dialog's group for a registered MIME type.
pub fn group_of_mime(mime: &str) -> FileGroup {
    let (top, sub) = mime.split_once('/').unwrap_or((mime, ""));
    let any = |hints: &[&str]| hints.iter().any(|h| sub.contains(h));
    match top {
        "text" | "application" if any(SUBTITLE_HINTS) => FileGroup::Video,
        "text" => FileGroup::Documents,
        "application" if sub == "rtf" => FileGroup::Documents,
        "video" => FileGroup::Video,
        "audio" => FileGroup::Sound,
        "model" => FileGroup::Models,
        "chemical" => FileGroup::Science,
        "font" => FileGroup::Fonts,
        "application" if sub.starts_with("x-font") => FileGroup::Fonts,
        "image" if any(RAW_HINTS) => FileGroup::CameraRaw,
        "image" if any(SCIENCE_IMAGE_HINTS) => FileGroup::Science,
        "image" if any(DRAWING_HINTS) => FileGroup::Drawings,
        "image" if sub.contains("djvu") => FileGroup::Documents,
        "image" => FileGroup::Images,
        "application" if any(DOCUMENT_HINTS) => FileGroup::Documents,
        "application" if any(DRAWING_HINTS) => FileGroup::Drawings,
        "application" if any(MAP_HINTS) => FileGroup::Maps,
        "application" if any(LAYERED_IMAGE_HINTS) => FileGroup::Images,
        "application" if any(MODEL_HINTS) => FileGroup::Models,
        "application" if sub == "mxf" || sub.contains("ms-asf") => FileGroup::Video,
        "application" if sub == "x-shorten" => FileGroup::Sound,
        "application" if sub.contains("xspf") => FileGroup::Other,
        // The rest of application/ is the data of some science: DICOM, NIfTI, HDF, NetCDF,
        // spectra, surface scans, thermal cameras.
        "application" => FileGroup::Science,
        _ => FileGroup::Other,
    }
}

impl super::Registry {
    /// The Open dialog's groups: each (in [`FileGroup::ALL`] order, empty ones left out)
    /// with its extensions in registry order. Every extension of
    /// [`Registry::extensions`](super::Registry::extensions) is in exactly one. The group
    /// follows the extension's MIME type; a camera raw decoder's extensions are camera raw
    /// whatever their type, and an unregistered extension (see [`EXTENSION_MIME`]) goes
    /// with its decoder's kind.
    pub fn file_groups(&self) -> Vec<(FileGroup, Vec<&'static str>)> {
        let mut by: Vec<(FileGroup, Vec<&'static str>)> =
            FileGroup::ALL.iter().map(|g| (*g, Vec::new())).collect();
        let mut seen: Vec<&'static str> = Vec::new();
        for d in &self.decoders {
            let format = d.format();
            for &ext in d.extensions() {
                if seen.contains(&ext) {
                    continue;
                }
                seen.push(ext);
                let g = if format == super::Format::Raw {
                    FileGroup::CameraRaw
                } else {
                    match mime_of_extension(ext) {
                        Some(Some(m)) => group_of_mime(m),
                        _ if format.is_mesh() || format.is_points() || format.is_splats() => {
                            FileGroup::Models
                        }
                        _ => FileGroup::Images,
                    }
                };
                if let Some((_, v)) = by.iter_mut().find(|(k, _)| *k == g) {
                    v.push(ext);
                }
            }
        }
        by.retain(|(_, v)| !v.is_empty());
        by
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every extension is in exactly one group, the groups cover them all, and the common
    /// kinds land where a person looks for them.
    #[test]
    fn file_groups_cover_every_extension_once() {
        let reg = crate::Registry::new();
        let groups = reg.file_groups();
        let mut all: Vec<&str> = groups.iter().flat_map(|(_, v)| v.iter().copied()).collect();
        assert_eq!(all.len(), reg.extensions().len());
        all.sort_unstable();
        let mut exts = reg.extensions();
        exts.sort_unstable();
        assert_eq!(all, exts);
        // In the dialog's order, none empty.
        let order: Vec<usize> = groups
            .iter()
            .map(|(g, _)| FileGroup::ALL.iter().position(|a| a == g).unwrap())
            .collect();
        assert!(order.windows(2).all(|w| w[0] < w[1]));
        assert!(groups.iter().all(|(_, v)| !v.is_empty()));
        let of = |e: &str| groups.iter().find(|(_, v)| v.contains(&e)).map(|(g, _)| *g);
        for (e, g) in [
            ("jpg", FileGroup::Images),
            ("png", FileGroup::Images),
            ("kra", FileGroup::Images),
            ("krz", FileGroup::Images),
            ("cr2", FileGroup::CameraRaw),
            ("dng", FileGroup::CameraRaw),
            ("nef", FileGroup::CameraRaw),
            ("mp4", FileGroup::Video),
            ("flac", FileGroup::Sound),
            ("pdf", FileGroup::Documents),
            ("epub", FileGroup::Documents),
            ("svg", FileGroup::Drawings),
            ("dxf", FileGroup::Drawings),
            ("stl", FileGroup::Models),
            ("gpx", FileGroup::Maps),
            ("fits", FileGroup::Science),
            ("dcm", FileGroup::Science),
            ("nii", FileGroup::Science),
            ("txt", FileGroup::Documents),
            ("md", FileGroup::Documents),
            ("csv", FileGroup::Documents),
            ("rtf", FileGroup::Documents),
            ("srt", FileGroup::Video),
            ("sup", FileGroup::Video),
            ("ttml", FileGroup::Video),
        ] {
            assert_eq!(of(e), Some(g), ".{e}");
        }
    }

    #[test]
    fn every_decoder_extension_has_an_entry() {
        let reg = crate::Registry::new();
        let missing: Vec<_> = reg
            .extensions()
            .into_iter()
            .filter(|e| mime_of_extension(e).is_none())
            .collect();
        assert!(
            missing.is_empty(),
            "add these to EXTENSION_MIME: {missing:?}"
        );
        // No stale rows either.
        let exts = reg.extensions();
        let stale: Vec<_> = EXTENSION_MIME
            .iter()
            .map(|(e, _)| *e)
            .filter(|e| !exts.contains(e))
            .collect();
        assert!(stale.is_empty(), "no decoder claims {stale:?}");
        let mut seen = std::collections::HashSet::new();
        assert!(EXTENSION_MIME.iter().all(|(e, _)| seen.insert(*e)));
    }

    #[test]
    fn groups() {
        let g = crate::Registry::new().mime_groups();
        assert!(g.contains(&(
            "image/jpeg",
            vec!["jpg", "jpeg", "jpe", "jfif", "jif", "thm", "insp"]
        )));
        assert!(
            g.iter()
                .any(|(m, e)| *m == "image/heif" && e == &["heif", "hif"])
        );
        assert!(
            !g.iter()
                .any(|(_, e)| e.contains(&"rs") || e.contains(&"vst"))
        );
        assert!(
            g.iter()
                .all(|(m, e)| m.contains('/') && (!e.is_empty() || CONTENT_MIME.contains(m)))
        );
        assert!(g.contains(&("image/x-smv", vec![])));
        // `.xyz` is a point cloud or a molecule: both types are registered.
        assert!(g.contains(&("model/x-xyz", vec!["xyz"])));
        assert!(g.contains(&("chemical/x-xyz", vec!["xyz"])));
        for (e, m) in SHARED_EXTENSION_MIME {
            assert!(matches!(mime_of_extension(e), Some(Some(p)) if p != *m));
        }
        // Industrial CT: headers registered, headerless samples not. A `.log` is plain text
        // (a SkyScan reconstruction log is recognised by content).
        assert!(g.contains(&("application/x-xradia-txm", vec!["txm", "txrm", "xrm"])));
        assert!(g.contains(&("application/x-xtek-ct", vec!["xtekct", "xtekvolume"])));
        assert!(
            !g.iter()
                .any(|(_, e)| e.contains(&"raw") || e.contains(&"vol"))
        );
        assert!(g.contains(&("text/x-log", vec!["log"])));
        // Text, tables and subtitles: one type each, VobSub's `.idx` left unregistered.
        assert!(g.contains(&("text/plain", vec!["txt", "text"])));
        assert!(g.contains(&("text/markdown", vec!["md", "markdown", "mdown", "mkd"])));
        assert!(g.contains(&("text/x-ssa", vec!["ass", "ssa"])));
        assert!(!g.iter().any(|(_, e)| e.contains(&"idx")));
        let text_types = g
            .iter()
            .filter(|(m, _)| {
                m.starts_with("text/")
                    || matches!(
                        *m,
                        "application/rtf"
                            | "application/x-subrip"
                            | "application/x-pgs"
                            | "application/ttml+xml"
                    )
            })
            .count();
        assert_eq!(text_types, 15);
    }
}
