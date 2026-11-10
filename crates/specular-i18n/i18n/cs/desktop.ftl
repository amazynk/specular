# machine-drafted, needs native review
# Specular — hluboká integrace s desktopem: Nastavení → Integrace, nastavení tapety. Čeština.

settings-section-integration = Integrace
settings-integration-default-viewer = Výchozí prohlížeč obrázků
settings-integration-default-viewer-help = Otevírat obrázky dvojitým kliknutím v aplikaci { -app-name }. Odebráním se obnoví dříve používaný prohlížeč.
# machine-drafted, needs native review
settings-integration-extra-default-types = Další typy souborů
settings-integration-extra-default-types-help = Další druhy souborů, které se dvojitým kliknutím otevřou v aplikaci { -app-name }. Volí se u Výchozího prohlížeče obrázků.
settings-integration-thumbnailer = Náhledy ve správci souborů
settings-integration-thumbnailer-help = Umožní správcům souborů zobrazovat náhledy formátů, které samy neumí přečíst, například JPEG XL, HEIC a soubory RAW z fotoaparátů.
settings-integration-single-instance = Jediná instance přes D-Bus
settings-integration-single-instance-help = Soubory otevřené ze správce souborů, spouštěče nebo terminálu se otevřou ve spuštěném okně aplikace { -app-name }. Platí pro později otevřená okna.
settings-integration-file-manager-actions = Akce ve správci souborů
settings-integration-file-manager-actions-help = Přidá položky „Otevřít“, „Porovnat“ a „Nastavit jako tapetu“ do místních nabídek aplikací Nautilus, Dolphin, Thunar a Nemo.
settings-integration-search-provider = Vyhledávání v desktopu
# machine-drafted, needs native review
settings-integration-search-provider-help = Hledejte obrázky z knihovny ve vyhledávání GNOME a KDE. Slova fungují jako ve vyhledávacím poli knihovny.
# machine-drafted, needs native review
settings-integration-url-scheme = Otevírat odkazy specular://
settings-integration-url-scheme-help = Odkazy jako specular:///home/me/photo.jpg v prohlížeči nebo jiném programu se otevřou v { -app-name }. Otevírají se jen soubory na tomto počítači a neobvyklé až po vašem potvrzení.

integration-state-on = Zapnuto
integration-state-off = Vypnuto
integration-state-partial = Nastaveno jen částečně. Opravíte opětovným použitím.
integration-state-unavailable = V tomto desktopu není k dispozici
integration-state-sandboxed = V izolovaném prostředí (Flatpak) není k dispozici
integration-state-no-bus = Není k dispozici: chybí relace D-Bus
integration-state-no-file-manager = Není k dispozici: nebyl nalezen podporovaný správce souborů
integration-state-no-search = Není k dispozici: nebyl nalezen GNOME Shell ani KRunner
integration-state-working = Probíhá…
integration-state-details = { $state } · { $details }
integration-detail-types = typy obrázků: { $count } z { $total }
integration-detail-thumbnail-types = typy obrázků: { $count }
integration-apply = Použít
integration-remove = Odebrat
integration-done-on = „{ $option }“ je zapnuto
integration-done-off = „{ $option }“ je vypnuto
integration-failed = Nelze změnit „{ $option }“: { $reason }
# machine-drafted, needs native review
integration-search-remove-ask = Vypnout vyhledávání v desktopu?
integration-search-remove-detail = { -app-name } bude odebrán z vyhledávání { $desktops }. Knihovna i obrázky zůstanou.
search-provider-open-folder = Otevřít složku

integration-label-open = Otevřít v aplikaci { -app-name }
integration-label-compare = Porovnat v aplikaci { -app-name }
integration-label-wallpaper = Nastavit jako tapetu
integration-label-generic-name = Prohlížeč obrázků
integration-label-comment = Okamžité otevírání a procházení velkých fotografií

palette-set-wallpaper = Nastavit jako tapetu
wallpaper-set = Tapeta nastavena ({ $method })
wallpaper-failed = Tapetu nelze nastavit: { $reason }

# machine-drafted, needs native review
url-invalid = Tento odkaz { -app-name } nelze otevřít: { $reason }
url-not-found = Soubory, které odkaz uvádí, neexistují
url-remote-refused = { -app-name } otevírá jen soubory na tomto počítači, ne { $source }
url-confirm-files = { $more ->
    [0] „{ $file }“
   *[other] „{ $file }“ a další (+{ $more })
}
url-confirm-title = { $reason ->
    [dots] Otevřít { url-confirm-files }? Odkaz k němu vede přes složky „..“.
    [network] Otevřít { url-confirm-files }? Je na síťovém úložišti nebo zařízení.
   *[hidden] Otevřít { url-confirm-files }? Název obsahuje neviditelné znaky nebo znaky směru textu.
}
url-confirm-open = Otevřít

# Launcher actions and recent files: machine-drafted, needs native review
integration-label-new-window = Nové okno
integration-label-library = Knihovna
integration-label-slideshow = Prezentace Obrázků
settings-integration-recent-files = Nedávné soubory v prostředí
settings-integration-recent-files-help = Otevřené obrázky se zobrazí v části „Nedávné“ ve správci souborů a v seznamech nedávných souborů prostředí. Odebráním se položky { -app-name } z těchto seznamů zase odstraní.
integration-detail-recent = soubory: { $count }
# machine-drafted, needs native review
integration-extra-title = Další typy souborů
# machine-drafted, needs native review
integration-extra-intro = Kromě obrázků může { -app-name } otevírat dvojitým kliknutím i tyto soubory. Nic není zaškrtnuto, dokud to nezaškrtnete. Programy, které tyto soubory upravují, zůstanou v nabídce „Otevřít pomocí“ a odškrtnutý typ se vrátí programu, který měl dříve. Blackmagic RAW, DWG, STEP, IGES a Zeiss CZI navíc potřebují bezplatný software od svých tvůrců: viz Nastavení › Volitelné dekodéry.
integration-extra-chosen = { $count ->
    [0] Nic nevybráno
    [one] Vybrán 1 typ
    [few] Vybrány { $count } typy
    [many] Vybráno { $count } typu
   *[other] Vybráno { $count } typů
}
integration-extra-count = { $on } z { $total }
integration-extra-none-yet = Zatím žádné
integration-extra-keywords = typy souborů formáty přidružení dvojklik otevřít pomocí editor věda medicína CAD GIS 3D molekuly video SVG PDF
integration-extra-group-editors = Soubory editorů
# machine-drafted, needs native review
integration-extra-group-editors-help = Dokumenty malířských a kreslicích programů: Krita, GIMP, Paint.NET, PaintShop Pro, Procreate, Affinity, OpenRaster, Illustrator, CorelDRAW, Clip Studio Paint, Sketch a Figma. Dvojklik zobrazí obrázek; upravujete ho dál v programu, který ho vytvořil.
integration-extra-group-science = Věda
integration-extra-group-science-help = Soubory z přístrojů a výzkumného softwaru: termokamery, mikroskopy, měření povrchu, HDF5, NetCDF, seismická a meteorologická data.
integration-extra-group-medical = Medicína
integration-extra-group-medical-help = Lékařské snímky a zobrazování mozku: DICOM, NIfTI, MINC, NRRD, traktografie a povrchy mozku.
integration-extra-group-cad-gis = CAD a GIS
# machine-drafted, needs native review
integration-extra-group-cad-gis-help = Plotterové výkresy, technické ilustrace, návrhy čipů a mapy: HP-GL, CGM, GDSII, shapefily, GeoPackage, KML, GPX, GeoJSON a výškové dlaždice. DWG se otevírá bezplatným ODA File Converter, STEP a IGES pomocí OpenCASCADE.
integration-extra-group-models = 3D a molekuly
integration-extra-group-models-help = 3D modely, modely budov, mračna bodů a molekuly: STL, OBJ, glTF, PLY, 3MF, X3D, IFC, LAS, E57, PDB, CIF a další.
integration-extra-group-videos = Videa
# machine-drafted, needs native review
integration-extra-group-videos-help = Videa: MP4, MOV, M4V, 3GP, 3G2, MKV, WebM, AVCHD (MTS, M2TS), OGV, IVF a MXF. Blackmagic RAW se otevírá bezplatným Blackmagic RAW SDK.
integration-extra-group-other-images = SVG a další obrázky
# machine-drafted, needs native review
integration-extra-group-other-images-help = Obrázky a dokumenty, které obvykle otevírají jiné programy: kresby SVG, PDF, XPS, DjVu, EPS, WMF, EMF, Visio, OpenDocument, WordPerfect Graphics, knihy EPUB, písma a animace jako MNG, FLIC a animované kurzory.
integration-extra-group-text = Text, tabulky a titulky
integration-extra-group-text-help = Prostý text a protokoly, NFO, Markdown, tabulky CSV a TSV, RTF a soubory titulků: SRT, WebVTT, ASS, SSA, MicroDVD, PGS, LRC, TTML a SCC.
