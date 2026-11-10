# machine-drafted, needs native review
# Integracja z pulpitem: Ustawienia → Integracja, ustawianie tapety. Polski.

settings-section-integration = Integracja
settings-integration-default-viewer = Domyślna przeglądarka obrazów
settings-integration-default-viewer-help = Otwieraj obrazy w programie { -app-name } po dwukrotnym kliknięciu. Wyłączenie przywraca poprzednią przeglądarkę.
# machine-drafted, needs native review
settings-integration-extra-default-types = Więcej typów plików
settings-integration-extra-default-types-help = Inne rodzaje plików otwierane w programie { -app-name } po dwukrotnym kliknięciu, wybierane w ustawieniu Domyślna przeglądarka obrazów.
settings-integration-thumbnailer = Miniatury w menedżerze plików
settings-integration-thumbnailer-help = Menedżery plików pokażą podglądy formatów, których same nie odczytują, np. JPEG XL, HEIC i plików RAW z aparatów.
settings-integration-single-instance = Jedna instancja przez D-Bus
settings-integration-single-instance-help = Pliki otwierane z menedżera plików, programu uruchamiającego lub terminala trafiają do działającego okna { -app-name }. Dotyczy okien otwartych później.
settings-integration-file-manager-actions = Akcje menedżera plików
settings-integration-file-manager-actions-help = Dodaj „Otwórz”, „Porównaj” i „Ustaw jako tapetę” do menu kontekstowych Nautilusa, Dolphina, Thunara i Nemo.
settings-integration-search-provider = Wyszukiwanie na pulpicie
# machine-drafted, needs native review
settings-integration-search-provider-help = Znajduj zdjęcia z biblioteki w wyszukiwaniu GNOME i KDE. Słowa działają jak w pasku wyszukiwania biblioteki.
# machine-drafted, needs native review
settings-integration-url-scheme = Otwieraj linki specular://
settings-integration-url-scheme-help = Linki takie jak specular:///home/me/photo.jpg w przeglądarce lub innym programie otwierają się w { -app-name }. Otwierane są tylko pliki z tego komputera, a nietypowe dopiero po potwierdzeniu.

integration-state-on = Włączone
integration-state-off = Wyłączone
integration-state-partial = Skonfigurowano częściowo. Zastosuj ponownie, aby naprawić.
integration-state-unavailable = Niedostępne w tym środowisku pulpitu
integration-state-sandboxed = Niedostępne w piaskownicy (Flatpak)
integration-state-no-bus = Niedostępne: brak sesji D-Bus
integration-state-no-file-manager = Niedostępne: nie znaleziono obsługiwanego menedżera plików
integration-state-no-search = Niedostępne: nie znaleziono GNOME Shell ani KRunnera
integration-state-working = Trwa praca…
integration-state-details = { $state } · { $details }
integration-detail-types = typy obrazów: { $count } z { $total }
integration-detail-thumbnail-types = typy obrazów: { $count }
integration-apply = Zastosuj
integration-remove = Usuń
integration-done-on = Włączono „{ $option }”
integration-done-off = Wyłączono „{ $option }”
integration-failed = Nie można zmienić „{ $option }”: { $reason }
# machine-drafted, needs native review
integration-search-remove-ask = Wyłączyć wyszukiwanie na pulpicie?
integration-search-remove-detail = { -app-name } zostanie usunięty z wyszukiwania { $desktops }. Biblioteka i zdjęcia zostają.
search-provider-open-folder = Otwórz folder

integration-label-open = Otwórz w { -app-name }
integration-label-compare = Porównaj w { -app-name }
integration-label-wallpaper = Ustaw jako tapetę
integration-label-generic-name = Przeglądarka obrazów
integration-label-comment = Błyskawicznie otwieraj i przeglądaj duże zdjęcia

palette-set-wallpaper = Ustaw jako tapetę
wallpaper-set = Ustawiono tapetę ({ $method })
wallpaper-failed = Nie można ustawić tapety: { $reason }

# machine-drafted, needs native review
url-invalid = Nie można otworzyć tego linku { -app-name }: { $reason }
url-not-found = Pliki wskazane przez ten link nie istnieją
url-remote-refused = { -app-name } otwiera tylko pliki z tego komputera, nie { $source }
url-confirm-files = { $more ->
    [0] „{ $file }”
   *[other] „{ $file }” i inne (+{ $more })
}
url-confirm-title = { $reason ->
    [dots] Otworzyć { url-confirm-files }? Link prowadzi przez foldery „..”.
    [network] Otworzyć { url-confirm-files }? Plik jest w udziale sieciowym lub na urządzeniu.
   *[hidden] Otworzyć { url-confirm-files }? Nazwa zawiera niewidoczne znaki lub znaki kierunku tekstu.
}
url-confirm-open = Otwórz

# Launcher actions and recent files: machine-drafted, needs native review
integration-label-new-window = Nowe okno
integration-label-library = Biblioteka
integration-label-slideshow = Pokaz slajdów z Obrazów
settings-integration-recent-files = Ostatnie pliki pulpitu
settings-integration-recent-files-help = Otwierane obrazy pojawiają się w „Ostatnich” w menedżerze plików i na listach ostatnich plików pulpitu. Usunięcie zabiera z tych list wpisy { -app-name }.
integration-detail-recent = pliki: { $count }
# machine-drafted, needs native review
integration-extra-title = Więcej typów plików
# machine-drafted, needs native review
integration-extra-intro = Oprócz obrazów { -app-name } może otwierać te pliki po dwukrotnym kliknięciu. Nic nie jest zaznaczone, dopóki tego nie zaznaczysz. Programy, które edytują te pliki, zostają w „Otwórz za pomocą”, a odznaczenie typu oddaje go programowi, który miał wcześniej. Blackmagic RAW, DWG, STEP, IGES i Zeiss CZI wymagają też darmowego oprogramowania ich twórców: zobacz Ustawienia › Opcjonalne dekodery.
integration-extra-chosen = { $count ->
    [0] Nic nie wybrano
    [one] Wybrano 1 typ
    [few] Wybrano { $count } typy
    [many] Wybrano { $count } typów
   *[other] Wybrano { $count } typu
}
integration-extra-count = { $on } z { $total }
integration-extra-none-yet = Jeszcze żadnych
integration-extra-keywords = typy plików formaty skojarzenia dwukrotne kliknięcie otwórz za pomocą edytor nauka medycyna CAD GIS 3D cząsteczki wideo SVG PDF
integration-extra-group-editors = Pliki edytorów
# machine-drafted, needs native review
integration-extra-group-editors-help = Dokumenty programów do malowania i rysowania: Krita, GIMP, Paint.NET, PaintShop Pro, Procreate, Affinity, OpenRaster, Illustrator, CorelDRAW, Clip Studio Paint, Sketch i Figma. Dwukrotne kliknięcie pokazuje obraz; edytujesz go nadal w programie, który go utworzył.
integration-extra-group-science = Nauka
integration-extra-group-science-help = Pliki z przyrządów i oprogramowania badawczego: kamery termowizyjne, mikroskopy, pomiary powierzchni, HDF5, NetCDF, dane sejsmiczne i pogodowe.
integration-extra-group-medical = Medycyna
integration-extra-group-medical-help = Badania obrazowe i obrazowanie mózgu: DICOM, NIfTI, MINC, NRRD, traktografia i powierzchnie mózgu.
integration-extra-group-cad-gis = CAD i GIS
# machine-drafted, needs native review
integration-extra-group-cad-gis-help = Rysunki z plotera, ilustracje techniczne, układy scalone i mapy: HP-GL, CGM, GDSII, shapefile, GeoPackage, KML, GPX, GeoJSON i kafle wysokości. DWG otwiera się za pomocą darmowego ODA File Converter, STEP i IGES za pomocą OpenCASCADE.
integration-extra-group-models = 3D i cząsteczki
integration-extra-group-models-help = Modele 3D, modele budynków, chmury punktów i cząsteczki: STL, OBJ, glTF, PLY, 3MF, X3D, IFC, LAS, E57, PDB, CIF i inne.
integration-extra-group-videos = Wideo
# machine-drafted, needs native review
integration-extra-group-videos-help = Wideo: MP4, MOV, M4V, 3GP, 3G2, MKV, WebM, AVCHD (MTS, M2TS), OGV, IVF i MXF. Blackmagic RAW otwiera się za pomocą darmowego Blackmagic RAW SDK.
integration-extra-group-other-images = SVG i inne obrazy
# machine-drafted, needs native review
integration-extra-group-other-images-help = Obrazy i dokumenty zwykle zostawiane innym programom: rysunki SVG, PDF, XPS, DjVu, EPS, WMF i EMF, Visio, OpenDocument, WordPerfect Graphics, książki EPUB i czcionki oraz animacje, takie jak MNG, FLIC i animowane kursory.
integration-extra-group-text = Tekst, tabele i napisy
integration-extra-group-text-help = Zwykły tekst i dzienniki, NFO, Markdown, tabele CSV i TSV, RTF oraz pliki napisów: SRT, WebVTT, ASS, SSA, MicroDVD, PGS, LRC, TTML i SCC.
