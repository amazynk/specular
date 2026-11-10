# Deep desktop integration: Settings → Integration, set as wallpaper.

settings-section-integration = Integration
settings-integration-default-viewer = Default image viewer
settings-integration-default-viewer-help = Open images with { -app-name } when you double-click them. Removing this restores the viewer you had before.
settings-integration-extra-default-types = More file types
settings-integration-extra-default-types-help = Other kinds of files to open with { -app-name } when you double-click them, chosen under Default image viewer.
settings-integration-thumbnailer = File-manager thumbnails
settings-integration-thumbnailer-help = Let file managers show previews of formats they cannot read themselves, such as JPEG XL, HEIC and camera raw files.
settings-integration-single-instance = Single instance through D-Bus
settings-integration-single-instance-help = Files opened from the file manager, a launcher or the terminal go to the running { -app-name } window. Applies to windows opened afterwards.
settings-integration-file-manager-actions = File-manager actions
settings-integration-file-manager-actions-help = Add “Open”, “Compare” and “Set as wallpaper” to the context menus of Nautilus, Dolphin, Thunar and Nemo.
settings-integration-search-provider = Desktop search
settings-integration-search-provider-help = Find pictures from your Library in the GNOME and KDE search. Words work as in the Library’s search bar.
settings-integration-url-scheme = Open specular:// links
settings-integration-url-scheme-help = Links such as specular:///home/me/photo.jpg in a browser or another program open in { -app-name }. Only files on this computer are opened, and unusual ones only after you confirm.
settings-integration-recent-files = Recent files in the desktop
settings-integration-recent-files-help = Images you open appear under “Recent” in the file manager and in the desktop’s recent-files lists. Removing this takes { -app-name }’s entries out of those lists again.

integration-state-on = On
integration-state-off = Off
integration-state-partial = Partly set up. Apply again to repair it.
integration-state-unavailable = Not available on this desktop
integration-state-sandboxed = Not available inside a sandbox (Flatpak)
integration-state-no-bus = Not available: there is no D-Bus session
integration-state-no-file-manager = Not available: no supported file manager was found
integration-state-no-search = Not available: neither GNOME Shell nor KRunner was found
integration-state-working = Working…
integration-state-details = { $state } · { $details }
integration-detail-types = image types: { $count } of { $total }
integration-detail-thumbnail-types = image types: { $count }
integration-detail-recent = files: { $count }
# Settings → Integration → Default image viewer → "More file types": tick boxes for kinds of
# files the default viewer leaves alone unless the user picks them.
integration-extra-title = More file types
integration-extra-intro = Besides pictures, { -app-name } can be the program that opens these when you double-click them. Nothing here is ticked unless you tick it. The programs that edit these files stay in “Open with”, and unticking a type gives it back to the program it had before. Blackmagic RAW, DWG, STEP, IGES and Zeiss CZI also need free software from their makers: see Settings › Optional decoders.
integration-extra-chosen = { $count ->
    [0] None chosen
    [one] 1 type chosen
   *[other] { $count } types chosen
}
integration-extra-count = { $on } of { $total }
integration-extra-none-yet = None yet
integration-extra-keywords = file types formats associations double-click open with editor science medical CAD GIS 3D molecules video SVG PDF
integration-extra-group-editors = Editor files
integration-extra-group-editors-help = Documents of painting and drawing programs: Krita, GIMP, Paint.NET, PaintShop Pro, Procreate, Affinity, OpenRaster, Illustrator, CorelDRAW, Clip Studio Paint, Sketch and Figma. Double-clicking shows the picture; you still edit it in the program that made it.
integration-extra-group-science = Science
integration-extra-group-science-help = Files from instruments and research software: thermal cameras, microscopes, surface measurements, HDF5, NetCDF, seismic and weather data.
integration-extra-group-medical = Medical
integration-extra-group-medical-help = Medical scans and brain imaging: DICOM, NIfTI, MINC, NRRD, tractography and brain surfaces.
integration-extra-group-cad-gis = CAD and GIS
integration-extra-group-cad-gis-help = Plotter drawings, technical illustrations, chip layouts and maps: HP-GL, CGM, GDSII, shapefiles, GeoPackage, KML, GPX, GeoJSON and elevation tiles. DWG opens with the free ODA File Converter, STEP and IGES with OpenCASCADE.
integration-extra-group-models = 3D and molecules
integration-extra-group-models-help = 3D models, building models, point clouds and molecules: STL, OBJ, glTF, PLY, 3MF, X3D, IFC, LAS, E57, PDB, CIF and more.
integration-extra-group-videos = Videos
integration-extra-group-videos-help = Videos: MP4, MOV, M4V, 3GP, 3G2, MKV, WebM, AVCHD (MTS, M2TS), OGV, IVF and MXF. Blackmagic RAW opens with the free Blackmagic RAW SDK.
integration-extra-group-other-images = SVG and other images
integration-extra-group-text = Text, tables and subtitles
integration-extra-group-text-help = Plain text and logs, NFO, Markdown, CSV and TSV tables, RTF, and subtitle files: SRT, WebVTT, ASS, SSA, MicroDVD, PGS, LRC, TTML and SCC.
integration-extra-group-other-images-help =Pictures and documents usually left to other programs: SVG drawings, PDF, XPS, DjVu, EPS, WMF and EMF, Visio, OpenDocument, WordPerfect Graphics, EPUB books and fonts, and animations such as MNG, FLIC and animated cursors.
integration-apply = Apply
integration-remove = Remove
integration-done-on = “{ $option }” is on
integration-done-off = “{ $option }” is off
integration-failed = Could not change “{ $option }”: { $reason }
integration-search-remove-ask = Turn off desktop search?
integration-search-remove-detail = { -app-name } is taken out of the search of { $desktops }. Your Library and pictures stay.
search-provider-open-folder = Open folder

integration-label-open = Open in { -app-name }
integration-label-compare = Compare in { -app-name }
integration-label-wallpaper = Set as Wallpaper
integration-label-generic-name = Image Viewer
integration-label-comment = Open and flip through large photos instantly
integration-label-new-window = New Window
integration-label-library = Library
integration-label-slideshow = Slideshow of Pictures

palette-set-wallpaper = Set as wallpaper
wallpaper-set = Wallpaper set ({ $method })
wallpaper-failed = Could not set the wallpaper: { $reason }

url-invalid = This { -app-name } link can't be opened: { $reason }
url-not-found = The files this link names don't exist
url-remote-refused = { -app-name } opens only files on this computer, not { $source }
url-confirm-files = { $more ->
    [0] “{ $file }”
   *[other] “{ $file }” and { $more } more
}
url-confirm-title = { $reason ->
    [dots] Open { url-confirm-files }? The link reaches it through “..” folders.
    [network] Open { url-confirm-files }? It is on a network share or a device.
   *[hidden] Open { url-confirm-files }? Its name contains invisible or text-direction characters.
}
url-confirm-open = Open
