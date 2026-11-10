# machine-drafted, needs native review
# Djup skrivbordsintegrering: Inställningar → Integrering, ange som skrivbordsbakgrund. Svenska.

settings-section-integration = Integrering
settings-integration-default-viewer = Standardbildvisare
settings-integration-default-viewer-help = Öppna bilder med { -app-name } när du dubbelklickar på dem. Tar du bort detta återställs den visare du hade tidigare.
# machine-drafted, needs native review
settings-integration-extra-default-types = Fler filtyper
settings-integration-extra-default-types-help = Andra filtyper som öppnas med { -app-name } när du dubbelklickar på dem. Väljs under Standardbildvisare.
settings-integration-thumbnailer = Miniatyrer i filhanteraren
settings-integration-thumbnailer-help = Låt filhanterare visa förhandsvisningar av format de inte kan läsa själva, till exempel JPEG XL, HEIC och RAW-filer från kameror.
settings-integration-single-instance = En instans via D-Bus
settings-integration-single-instance-help = Filer som öppnas från filhanteraren, en programstartare eller terminalen hamnar i det öppna { -app-name }-fönstret. Gäller fönster som öppnas efteråt.
settings-integration-file-manager-actions = Åtgärder i filhanteraren
settings-integration-file-manager-actions-help = Lägg till ”Öppna”, ”Jämför” och ”Ange som skrivbordsbakgrund” i snabbmenyerna i Nautilus, Dolphin, Thunar och Nemo.
settings-integration-search-provider = Skrivbordssökning
# machine-drafted, needs native review
settings-integration-search-provider-help = Hitta bilder från ditt bibliotek i sökningen i GNOME och KDE. Ord fungerar som i bibliotekets sökfält.
# machine-drafted, needs native review
settings-integration-url-scheme = Öppna specular://-länkar
settings-integration-url-scheme-help = Länkar som specular:///home/me/photo.jpg i en webbläsare eller ett annat program öppnas i { -app-name }. Bara filer på den här datorn öppnas, och ovanliga först när du bekräftat.

integration-state-on = På
integration-state-off = Av
integration-state-partial = Delvis konfigurerad. Verkställ igen för att reparera.
integration-state-unavailable = Inte tillgängligt på det här skrivbordet
integration-state-sandboxed = Inte tillgängligt i en sandlåda (Flatpak)
integration-state-no-bus = Inte tillgängligt: det finns ingen D-Bus-session
integration-state-no-file-manager = Inte tillgängligt: ingen filhanterare som stöds hittades
integration-state-no-search = Inte tillgängligt: varken GNOME Shell eller KRunner hittades
integration-state-working = Arbetar…
integration-state-details = { $state } · { $details }
integration-detail-types = bildtyper: { $count } av { $total }
integration-detail-thumbnail-types = bildtyper: { $count }
integration-apply = Verkställ
integration-remove = Ta bort
integration-done-on = ”{ $option }” är på
integration-done-off = ”{ $option }” är av
integration-failed = Det gick inte att ändra ”{ $option }”: { $reason }
# machine-drafted, needs native review
integration-search-remove-ask = Stänga av skrivbordssökning?
integration-search-remove-detail = { -app-name } tas bort från sökningen i { $desktops }. Ditt bibliotek och dina bilder finns kvar.
search-provider-open-folder = Öppna mapp

integration-label-open = Öppna i { -app-name }
integration-label-compare = Jämför i { -app-name }
integration-label-wallpaper = Ange som skrivbordsbakgrund
integration-label-generic-name = Bildvisare
integration-label-comment = Öppna och bläddra snabbt bland stora foton

palette-set-wallpaper = Ange som skrivbordsbakgrund
wallpaper-set = Skrivbordsbakgrunden har angetts ({ $method })
wallpaper-failed = Det gick inte att ange skrivbordsbakgrunden: { $reason }

# machine-drafted, needs native review
url-invalid = Den här { -app-name }-länken kan inte öppnas: { $reason }
url-not-found = Filerna som länken anger finns inte
url-remote-refused = { -app-name } öppnar bara filer på den här datorn, inte { $source }
url-confirm-files = { $more ->
    [0] ”{ $file }”
   *[other] ”{ $file }” och { $more } till
}
url-confirm-title = { $reason ->
    [dots] Öppna { url-confirm-files }? Länken når den via ”..”-mappar.
    [network] Öppna { url-confirm-files }? Den ligger på en nätverksresurs eller en enhet.
   *[hidden] Öppna { url-confirm-files }? Namnet innehåller osynliga tecken eller textriktningstecken.
}
url-confirm-open = Öppna

# Launcher actions and recent files: machine-drafted, needs native review
integration-label-new-window = Nytt fönster
integration-label-library = Bibliotek
integration-label-slideshow = Bildspel av Bilder
settings-integration-recent-files = Senaste filer på skrivbordet
settings-integration-recent-files-help = Bilder du öppnar visas under ”Senaste” i filhanteraren och i skrivbordets listor över senaste filer. Tas detta bort försvinner { -app-name }s poster ur listorna igen.
integration-detail-recent = filer: { $count }
# machine-drafted, needs native review
integration-extra-title = Fler filtyper
# machine-drafted, needs native review
integration-extra-intro = Förutom bilder kan { -app-name } öppna de här filerna när du dubbelklickar på dem. Inget är markerat förrän du markerar det. Programmen som redigerar filerna finns kvar under ”Öppna med”, och avmarkerar du en typ får den tillbaka sitt tidigare program. Blackmagic RAW, DWG, STEP, IGES och Zeiss CZI behöver dessutom gratis programvara från sina tillverkare: se Inställningar › Valfria avkodare.
integration-extra-chosen = { $count ->
    [0] Inga valda
    [one] 1 typ vald
   *[other] { $count } typer valda
}
integration-extra-count = { $on } av { $total }
integration-extra-none-yet = Inga ännu
integration-extra-keywords = filtyper format filassociationer dubbelklicka öppna med redigerare vetenskap medicinsk CAD GIS 3D molekyler video SVG PDF
integration-extra-group-editors = Redigeringsfiler
# machine-drafted, needs native review
integration-extra-group-editors-help = Dokument från mål- och ritprogram: Krita, GIMP, Paint.NET, PaintShop Pro, Procreate, Affinity, OpenRaster, Illustrator, CorelDRAW, Clip Studio Paint, Sketch och Figma. Dubbelklick visar bilden; du redigerar den fortfarande i programmet som skapade den.
integration-extra-group-science = Vetenskap
integration-extra-group-science-help = Filer från instrument och forskningsprogram: värmekameror, mikroskop, ytmätningar, HDF5, NetCDF, seismiska data och väderdata.
integration-extra-group-medical = Medicinskt
integration-extra-group-medical-help = Medicinska undersökningar och hjärnavbildning: DICOM, NIfTI, MINC, NRRD, traktografi och hjärnytor.
integration-extra-group-cad-gis = CAD och GIS
# machine-drafted, needs native review
integration-extra-group-cad-gis-help = Plotterritningar, tekniska illustrationer, kretslayouter och kartor: HP-GL, CGM, GDSII, shapefiler, GeoPackage, KML, GPX, GeoJSON och höjdrutor. DWG öppnas med gratisprogrammet ODA File Converter, STEP och IGES med OpenCASCADE.
integration-extra-group-models = 3D och molekyler
integration-extra-group-models-help = 3D-modeller, byggnadsmodeller, punktmoln och molekyler: STL, OBJ, glTF, PLY, 3MF, X3D, IFC, LAS, E57, PDB, CIF med flera.
integration-extra-group-videos = Videor
# machine-drafted, needs native review
integration-extra-group-videos-help = Videor: MP4, MOV, M4V, 3GP, 3G2, MKV, WebM, AVCHD (MTS, M2TS), OGV, IVF och MXF. Blackmagic RAW öppnas med det kostnadsfria Blackmagic RAW SDK.
integration-extra-group-other-images = SVG och andra bilder
# machine-drafted, needs native review
integration-extra-group-other-images-help = Bilder och dokument som oftast lämnas åt andra program: SVG-ritningar, PDF, XPS, DjVu, EPS, WMF och EMF, Visio, OpenDocument, WordPerfect Graphics, EPUB-böcker och typsnitt samt animationer som MNG, FLIC och animerade muspekare.
integration-extra-group-text = Text, tabeller och undertexter
integration-extra-group-text-help = Ren text och loggar, NFO, Markdown, CSV- och TSV-tabeller, RTF och undertextfiler: SRT, WebVTT, ASS, SSA, MicroDVD, PGS, LRC, TTML och SCC.
