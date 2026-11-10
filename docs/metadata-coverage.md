# Metadata coverage

What each supported format can carry, what the Info panel shows, and what is still missing.
Checked against exiftool 13.50 on one sample per extension (corpus files, test fixtures and
files generated with ffmpeg), 2026-11-03. Formats are grouped by family; every extension in
`specular_decode::mime::EXTENSION_MIME` belongs to one row.

"Shows" lists the Info panel groups that the metadata fills (File and Image are always
there). Formats without a metadata block of their own (BMP, QOI, PNM, TGA, ICO, DDS, KTX,
ASTC, XBM, farbfeld, Sun raster, SGI, PCX, FLI…) carry nothing to show and are left out.

## Photos and raster images

| Format | Carries | Shows | Gap |
|---|---|---|---|
| JPEG (jpg, jpeg, jpe, jfif, mpo) | Exif, maker notes, XMP, ICC, IPTC, COM comment, C2PA | Camera, Exposure, Lens, Date, Location, Rights (with IPTC title, caption, creator, copyright, keywords, date), Comment, all fields incl. an IPTC group | Maker notes other than Canon, Panasonic, Samsung, Fujifilm show as a blob |
| PNG, APNG | eXIf / Raw profile Exif, iTXt XMP, iCCP, tEXt / zTXt / iTXt text | Exif groups, XMP, ICC; text chunks: Title, Author, Description, Copyright, Creation Time, Software, Comment, and a "PNG tags" field group | — |
| GIF | Comment extension, XMP application extension | Comment, XMP (title, creator…) | — |
| WebP | EXIF, XMP, ICCP chunks | All three | — |
| HEIC / HEIF / AVIF | Exif item, XMP item, colr ICC | All three | Sequences (heics) with only track dates: none shown (exiftool: zero dates) |
| JPEG XL | Exif / brob Exif, xml box, codestream ICC | All three | — |
| JPEG 2000 (jp2, jph, jpx; j2k, j2c codestreams) | GeoJP2 / Exif uuid, XML boxes, COM marker | Geospatial group | Codestream COM comments ("Created by OpenJPEG") not shown |
| JPEG XR | IFD0 Exif, XMP, ICC | All three | — |
| TIFF, BigTIFF, GeoTIFF, OME/pyramid TIFF | IFD0, Exif, GPS, XMP, ICC, IPTC (33723 or inside 34377) | All, IPTC added | — |
| PSD / PSB | Exif, XMP, ICC, IPTC (resource 1028) | All, IPTC added | Other Photoshop resources (slices, guides) only as exiftool lists them |
| BPG, FLIF | Exif, XMP, ICC extensions | All three | — |
| PSP, TUB (Paint Shop Pro) | Creator chunk (title, artist, copyright, description, dates) | Image only | Creator chunk not read (long tail) |
| XCF, ORA, KRA, PDN, CLIP, Procreate, Affinity | Program-specific parasites, ZIP entries | Image only | Long tail |
| OpenEXR, Radiance HDR | Header attributes (owner, comments, capDate, software) | Image only | EXR/HDR header attributes not shown (long tail) |
| DPX, Cineon, XWD, MIFF, TIM, WSQ, JBIG, JBIG2, JPEG XS | Header text fields | File header group | — |
| SVG | `<title>`, `<desc>`, RDF metadata | Image only | Title and RDF metadata not shown (long tail) |
| DICOM | Data set | Medical image group (no patient attributes, by design) | — |

## Camera RAW

| Format | Carries | Shows | Gap |
|---|---|---|---|
| RAF (Fujifilm) | RAF header, preview JPEG Exif + Fujifilm maker note, FujiIFD | Camera, Exposure (focus mode, white balance, shutter type, drive mode), Lens (stabilisation), Picture settings (film simulation, dynamic range, highlights, shadows, colour, sharpness, noise reduction, clarity, grain, colour chrome), Date, Location, Rating, a "Fujifilm maker note" field group | RAF header (version, raw crop) and FujiIFD (black levels, WB levels) not shown; WhiteBalance 3840 reads "Custom 1" (exiftool "Custom") |
| DNG, CR2, CR3, NEF, NRW, ARW, SRF, SR2, ORF, RW2, RWL, PEF, SRW, MRW, 3FR, FFF, IIQ, MOS, MEF, DCR, KDC, ERF, GPR | Exif, GPS, XMP, maker notes | Camera, Exposure, Lens, Date, Location, Rights | Maker notes beyond Canon, Panasonic, Samsung, Fujifilm (Nikon, Sony, Olympus, Pentax picture settings) not decoded; Kodak ISO lives only in its maker note |
| CRW (Canon CIFF) | CIFF records | Camera, Exposure, Date (new: from the RAW decoder's facts) | Canon CIFF records beyond those not shown |
| X3F (Sigma) | Property list, embedded Exif | Camera, Exposure, Date (new) | Exposure from the property list: 1/558 s, f/6.4 where exiftool reads the JPEG Exif's 1/500 s, f/6.3 |
| BRAW, R3D | SDK metadata | Header group (BRAW via SDK) | — |

## Video

| Format | Carries | Shows | Gap |
|---|---|---|---|
| MP4, M4V, MOV, 3GP | mvhd dates, QuickTime keys / iTunes items / udta atoms (title, artist, comment, copyright, encoder, ©xyz location), XMP | Movie group (date, camera); new: Rights (title, creator, copyright, comment), Location from ©xyz / ISO 6709, Software, XMP, a "QuickTime tags" field group | — |
| MKV, WebM | Info title / writing app, Tags | New: Title, creator, copyright, comment, software, a "Matroska tags" field group | Track- and chapter-level tags left out on purpose |
| AVI | RIFF INFO list | New: as above | — |
| WMV / ASF | Content Description object, Extended Content Description | New: title, author, copyright | Extended Content Description (WM/… attributes) not read |
| OGV | Vorbis comments | New: as above (shared sound tag reader) | — |
| MPEG-TS, M2TS, MTS, MPEG-PS, VOB, MPG | AVCHD maker data (date, camera) | Movie group date / camera | — |
| MXF | Identification set (company, product) | Movie group camera | Package names not shown (long tail) |
| FLV | onMetaData (encoder) | — | Long tail |
| DV | Recording date | Movie group date | — |

## Sound

| Format | Carries | Shows | Gap |
|---|---|---|---|
| MP3, AAC, FLAC, Ogg Vorbis, Opus, Speex (new: its comment packet), WAV, AIFF, WavPack, APE, Musepack, M4A, MKA, DTS, AC-3 | ID3v1/v2, APE, Vorbis comments, iTunes items, RIFF INFO, AIFF text, Matroska tags | Sound group (title, artist, album, composer, genre, date, track, disc, comment, copyright, encoder, gain, cover) | — |
| AMR | — | — | — |

## Documents, books, fonts

| Format | Carries | Shows | Gap |
|---|---|---|---|
| PDF, AI, EPS | Info dictionary, XMP | Rights and File (title, creator, application) from XMP or the info dictionary | Creation date not shown when the catalogue XMP lacks it |
| Word, Excel, PowerPoint, Visio, Project, OpenDocument, iWork | Document properties | Document group | — |
| EPUB, MOBI / AZW3, FB2 | OPF / EXTH / description | Book group | — |
| TTF, OTF, TTC, OTC, WOFF, WOFF2 | name table | Font group | Font description string not shown |
| DjVu, XPS | Annotations, core properties | Page only | Long tail |

## Science, medical, geospatial, 3D

These formats have their own Info groups (Scientific image, Microscopy, Diffraction, Spectrum,
Mass spectrometry, Thermal image, Volume, Geospatial, Point cloud, 3D model, Structure,
Drawing…) built from their headers; exiftool reads little or nothing from most of them. The
FLIR (FFF, SEQ, CSQ) camera records and thermal parameters show in the Thermal image group.

## How the gaps were closed

- `specular-metadata`: a Fujifilm maker-note reader (`fuji.rs`), an IPTC-IIM reader
  (`iptc.rs`), and `MetadataInput::iptc` / `MetadataInput::tags`: a container's tags arrive
  as XMP-equivalent properties, which every summary group reads where the XMP has none, and
  as an "all fields" group of their own.
- `specular-decode`: IPTC from TIFF and PSD (`embedded_metadata`), PNG and GIF text tags and
  GIF XMP (`text_tags`), movie tag mapping (`text_tags::movie_tags`), and
  `CameraInfo::exif_tiff` for RAWs whose metadata is not Exif.
- `specular-video`: every movie text tag kept (`Movie::tags`), Matroska and ASF tags, AVI
  INFO; `VideoFacts::tags` and `VideoFacts::xmp` reach the Info panel.
