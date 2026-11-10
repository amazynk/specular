# machine-drafted, needs native review
# Dyb skrivebordsintegration: Indstillinger → Integration, brug som baggrund. Dansk.

settings-section-integration = Integration
settings-integration-default-viewer = Standardbilledfremviser
settings-integration-default-viewer-help = Åbn billeder med { -app-name }, når du dobbeltklikker på dem. Hvis du fjerner dette, gendannes den fremviser, du havde før.
# machine-drafted, needs native review
settings-integration-extra-default-types = Flere filtyper
settings-integration-extra-default-types-help = Andre filtyper, der åbnes med { -app-name }, når du dobbeltklikker på dem. Vælges under Standardbilledfremviser.
settings-integration-thumbnailer = Miniaturer i filhåndteringen
settings-integration-thumbnailer-help = Lad filhåndteringer vise eksempler på formater, de ikke selv kan læse, f.eks. JPEG XL, HEIC og RAW-filer fra kameraer.
settings-integration-single-instance = Én forekomst via D-Bus
settings-integration-single-instance-help = Filer, der åbnes fra filhåndteringen, en starter eller terminalen, sendes til det kørende { -app-name }-vindue. Gælder for vinduer, der åbnes herefter.
settings-integration-file-manager-actions = Handlinger i filhåndteringen
settings-integration-file-manager-actions-help = Føj ”Åbn”, ”Sammenlign” og ”Brug som baggrund” til genvejsmenuerne i Nautilus, Dolphin, Thunar og Nemo.
settings-integration-search-provider = Skrivebordssøgning
# machine-drafted, needs native review
settings-integration-search-provider-help = Find billeder fra dit bibliotek i GNOME- og KDE-søgningen. Ord virker som i bibliotekets søgefelt.
# machine-drafted, needs native review
settings-integration-url-scheme = Åbn specular://-links
settings-integration-url-scheme-help = Links som specular:///home/me/photo.jpg i en browser eller et andet program åbnes i { -app-name }. Kun filer på denne computer åbnes, og usædvanlige først når du har bekræftet.

integration-state-on = Til
integration-state-off = Fra
integration-state-partial = Delvist konfigureret. Anvend igen for at reparere det.
integration-state-unavailable = Ikke tilgængelig på dette skrivebord
integration-state-sandboxed = Ikke tilgængelig i en sandkasse (Flatpak)
integration-state-no-bus = Ikke tilgængelig: der er ingen D-Bus-session
integration-state-no-file-manager = Ikke tilgængelig: der blev ikke fundet nogen understøttet filhåndtering
integration-state-no-search = Ikke tilgængelig: hverken GNOME Shell eller KRunner blev fundet
integration-state-working = Arbejder…
integration-state-details = { $state } · { $details }
integration-detail-types = billedtyper: { $count } af { $total }
integration-detail-thumbnail-types = billedtyper: { $count }
integration-apply = Anvend
integration-remove = Fjern
integration-done-on = ”{ $option }” er slået til
integration-done-off = ”{ $option }” er slået fra
integration-failed = Kunne ikke ændre ”{ $option }”: { $reason }
# machine-drafted, needs native review
integration-search-remove-ask = Slå skrivebordssøgning fra?
integration-search-remove-detail = { -app-name } fjernes fra søgningen i { $desktops }. Dit bibliotek og dine billeder bliver.
search-provider-open-folder = Åbn mappe

integration-label-open = Åbn i { -app-name }
integration-label-compare = Sammenlign i { -app-name }
integration-label-wallpaper = Brug som baggrund
integration-label-generic-name = Billedfremviser
integration-label-comment = Åbn og bladr gennem store fotos med det samme

palette-set-wallpaper = Brug som baggrund
wallpaper-set = Baggrunden er angivet ({ $method })
wallpaper-failed = Kunne ikke angive baggrunden: { $reason }

# machine-drafted, needs native review
url-invalid = Dette { -app-name }-link kan ikke åbnes: { $reason }
url-not-found = Filerne, som linket nævner, findes ikke
url-remote-refused = { -app-name } åbner kun filer på denne computer, ikke { $source }
url-confirm-files = { $more ->
    [0] “{ $file }”
   *[other] “{ $file }” og { $more } mere
}
url-confirm-title = { $reason ->
    [dots] Åbn { url-confirm-files }? Linket når den via “..”-mapper.
    [network] Åbn { url-confirm-files }? Den ligger på et netværksdrev eller en enhed.
   *[hidden] Åbn { url-confirm-files }? Navnet indeholder usynlige tegn eller tekstretningstegn.
}
url-confirm-open = Åbn

# Launcher actions and recent files: machine-drafted, needs native review
integration-label-new-window = Nyt vindue
integration-label-library = Bibliotek
integration-label-slideshow = Diasshow af Billeder
settings-integration-recent-files = Seneste filer på skrivebordet
settings-integration-recent-files-help = Billeder, du åbner, vises under “Seneste” i filhåndteringen og i skrivebordets lister over seneste filer. Fjernes dette, tages { -app-name }s poster ud af listerne igen.
integration-detail-recent = filer: { $count }
# machine-drafted, needs native review
integration-extra-title = Flere filtyper
# machine-drafted, needs native review
integration-extra-intro = Ud over billeder kan { -app-name } åbne disse filer, når du dobbeltklikker på dem. Intet er markeret, før du selv markerer det. Programmerne, der redigerer filerne, bliver i ”Åbn med”, og fjerner du markeringen, får typen sit tidligere program igen. Blackmagic RAW, DWG, STEP, IGES og Zeiss CZI kræver desuden gratis software fra deres producenter: se Indstillinger › Valgfrie dekodere.
integration-extra-chosen = { $count ->
    [0] Ingen valgt
    [one] 1 type valgt
   *[other] { $count } typer valgt
}
integration-extra-count = { $on } af { $total }
integration-extra-none-yet = Ingen endnu
integration-extra-keywords = filtyper formater filtilknytninger dobbeltklik åbn med editor videnskab medicinsk CAD GIS 3D molekyler video SVG PDF
integration-extra-group-editors = Editorfiler
# machine-drafted, needs native review
integration-extra-group-editors-help = Dokumenter fra tegne- og maleprogrammer: Krita, GIMP, Paint.NET, PaintShop Pro, Procreate, Affinity, OpenRaster, Illustrator, CorelDRAW, Clip Studio Paint, Sketch og Figma. Dobbeltklik viser billedet; du redigerer det stadig i programmet, der lavede det.
integration-extra-group-science = Videnskab
integration-extra-group-science-help = Filer fra instrumenter og forskningssoftware: termokameraer, mikroskoper, overflademålinger, HDF5, NetCDF, seismiske data og vejrdata.
integration-extra-group-medical = Medicinsk
integration-extra-group-medical-help = Medicinske scanninger og hjernebilleddannelse: DICOM, NIfTI, MINC, NRRD, traktografi og hjerneoverflader.
integration-extra-group-cad-gis = CAD og GIS
# machine-drafted, needs native review
integration-extra-group-cad-gis-help = Plottertegninger, tekniske illustrationer, chiplayouts og kort: HP-GL, CGM, GDSII, shapefiler, GeoPackage, KML, GPX, GeoJSON og højdefliser. DWG åbnes med det gratis ODA File Converter, STEP og IGES med OpenCASCADE.
integration-extra-group-models = 3D og molekyler
integration-extra-group-models-help = 3D-modeller, bygningsmodeller, punktskyer og molekyler: STL, OBJ, glTF, PLY, 3MF, X3D, IFC, LAS, E57, PDB, CIF og flere.
integration-extra-group-videos = Videoer
# machine-drafted, needs native review
integration-extra-group-videos-help = Videoer: MP4, MOV, M4V, 3GP, 3G2, MKV, WebM, AVCHD (MTS, M2TS), OGV, IVF og MXF. Blackmagic RAW åbnes med det gratis Blackmagic RAW SDK.
integration-extra-group-other-images = SVG og andre billeder
# machine-drafted, needs native review
integration-extra-group-other-images-help = Billeder og dokumenter, der normalt overlades til andre programmer: SVG-tegninger, PDF, XPS, DjVu, EPS, WMF og EMF, Visio, OpenDocument, WordPerfect Graphics, EPUB-bøger og skrifttyper samt animationer som MNG, FLIC og animerede markører.
integration-extra-group-text = Tekst, tabeller og undertekster
integration-extra-group-text-help = Ren tekst og logfiler, NFO, Markdown, CSV- og TSV-tabeller, RTF og undertekstfiler: SRT, WebVTT, ASS, SSA, MicroDVD, PGS, LRC, TTML og SCC.
