# machine-drafted, needs native review
# Diepe bureaubladintegratie: Instellingen → Integratie, als achtergrond instellen. Nederlands.

settings-section-integration = Integratie
settings-integration-default-viewer = Standaardafbeeldingsviewer
settings-integration-default-viewer-help = Afbeeldingen openen met { -app-name } als je erop dubbelklikt. Als je dit verwijdert, krijg je je vorige viewer terug.
# machine-drafted, needs native review
settings-integration-extra-default-types = Meer bestandstypen
settings-integration-extra-default-types-help = Andere soorten bestanden die met { -app-name } openen als je erop dubbelklikt, gekozen onder Standaardafbeeldingsviewer.
settings-integration-thumbnailer = Miniaturen in bestandsbeheer
settings-integration-thumbnailer-help = Laat bestandsbeheerders voorbeelden tonen van indelingen die ze zelf niet kunnen lezen, zoals JPEG XL, HEIC en RAW-bestanden van camera’s.
settings-integration-single-instance = Eén exemplaar via D-Bus
settings-integration-single-instance-help = Bestanden die je opent vanuit bestandsbeheer, een starter of de terminal gaan naar het geopende { -app-name }-venster. Geldt voor vensters die daarna worden geopend.
settings-integration-file-manager-actions = Acties in bestandsbeheer
settings-integration-file-manager-actions-help = Voeg ‘Openen’, ‘Vergelijken’ en ‘Als achtergrond instellen’ toe aan de contextmenu’s van Nautilus, Dolphin, Thunar en Nemo.
settings-integration-search-provider = Zoeken op het bureaublad
# machine-drafted, needs native review
settings-integration-search-provider-help = Vind afbeeldingen uit je bibliotheek via zoeken in GNOME en KDE. Woorden werken zoals in de zoekbalk van de bibliotheek.
# machine-drafted, needs native review
settings-integration-url-scheme = specular://-links openen
settings-integration-url-scheme-help = Links zoals specular:///home/me/photo.jpg in een browser of ander programma openen in { -app-name }. Alleen bestanden op deze computer worden geopend, en ongebruikelijke pas nadat u bevestigt.

integration-state-on = Aan
integration-state-off = Uit
integration-state-partial = Gedeeltelijk ingesteld. Pas opnieuw toe om het te herstellen.
integration-state-unavailable = Niet beschikbaar op dit bureaublad
integration-state-sandboxed = Niet beschikbaar in een sandbox (Flatpak)
integration-state-no-bus = Niet beschikbaar: er is geen D-Bus-sessie
integration-state-no-file-manager = Niet beschikbaar: geen ondersteund bestandsbeheer gevonden
integration-state-no-search = Niet beschikbaar: GNOME Shell en KRunner zijn niet gevonden
integration-state-working = Bezig…
integration-state-details = { $state } · { $details }
integration-detail-types = afbeeldingstypen: { $count } van { $total }
integration-detail-thumbnail-types = afbeeldingstypen: { $count }
integration-apply = Toepassen
integration-remove = Verwijderen
integration-done-on = ‘{ $option }’ staat aan
integration-done-off = ‘{ $option }’ staat uit
integration-failed = Kan ‘{ $option }’ niet wijzigen: { $reason }
# machine-drafted, needs native review
integration-search-remove-ask = Zoeken op het bureaublad uitzetten?
integration-search-remove-detail = { -app-name } wordt uit het zoeken van { $desktops } gehaald. Je bibliotheek en afbeeldingen blijven.
search-provider-open-folder = Map openen

integration-label-open = Openen in { -app-name }
integration-label-compare = Vergelijken in { -app-name }
integration-label-wallpaper = Als achtergrond instellen
integration-label-generic-name = Afbeeldingsviewer
integration-label-comment = Grote foto’s direct openen en doorbladeren

palette-set-wallpaper = Als achtergrond instellen
wallpaper-set = Achtergrond ingesteld ({ $method })
wallpaper-failed = Kan de achtergrond niet instellen: { $reason }

# machine-drafted, needs native review
url-invalid = Deze { -app-name }-link kan niet worden geopend: { $reason }
url-not-found = De bestanden die deze link noemt bestaan niet
url-remote-refused = { -app-name } opent alleen bestanden op deze computer, niet { $source }
url-confirm-files = { $more ->
    [0] ‘{ $file }’
   *[other] ‘{ $file }’ en nog { $more }
}
url-confirm-title = { $reason ->
    [dots] { url-confirm-files } openen? De link bereikt het via ‘..’-mappen.
    [network] { url-confirm-files } openen? Het staat op een netwerkshare of apparaat.
   *[hidden] { url-confirm-files } openen? De naam bevat onzichtbare of tekstrichtingstekens.
}
url-confirm-open = Openen

# Launcher actions and recent files: machine-drafted, needs native review
integration-label-new-window = Nieuw venster
integration-label-library = Bibliotheek
integration-label-slideshow = Diavoorstelling van Afbeeldingen
settings-integration-recent-files = Recente bestanden in het bureaublad
settings-integration-recent-files-help = Geopende afbeeldingen verschijnen onder ‘Recent’ in de bestandsbeheerder en in de lijsten met recente bestanden van het bureaublad. Bij verwijderen worden de items van { -app-name } weer uit die lijsten gehaald.
integration-detail-recent = bestanden: { $count }
# machine-drafted, needs native review
integration-extra-title = Meer bestandstypen
# machine-drafted, needs native review
integration-extra-intro = Naast afbeeldingen kan { -app-name } ook deze bestanden openen als je erop dubbelklikt. Niets is aangevinkt tenzij jij het aanvinkt. De programma’s die deze bestanden bewerken blijven in ‘Openen met’, en een type uitvinken geeft het terug aan het programma van eerst. Blackmagic RAW, DWG, STEP, IGES en Zeiss CZI hebben ook gratis software van hun makers nodig: zie Instellingen › Optionele decoders.
integration-extra-chosen = { $count ->
    [0] Niets gekozen
    [one] 1 type gekozen
   *[other] { $count } typen gekozen
}
integration-extra-count = { $on } van { $total }
integration-extra-none-yet = Nog geen
integration-extra-keywords = bestandstypen formaten koppelingen dubbelklikken openen met editor wetenschap medisch CAD GIS 3D moleculen video SVG PDF
integration-extra-group-editors = Editorbestanden
# machine-drafted, needs native review
integration-extra-group-editors-help = Documenten van teken- en schilderprogramma’s: Krita, GIMP, Paint.NET, PaintShop Pro, Procreate, Affinity, OpenRaster, Illustrator, CorelDRAW, Clip Studio Paint, Sketch en Figma. Dubbelklikken toont de afbeelding; bewerken doe je nog steeds in het programma dat hem maakte.
integration-extra-group-science = Wetenschap
integration-extra-group-science-help = Bestanden van meetinstrumenten en onderzoekssoftware: warmtecamera’s, microscopen, oppervlaktemetingen, HDF5, NetCDF, seismische en weergegevens.
integration-extra-group-medical = Medisch
integration-extra-group-medical-help = Medische scans en hersenbeeldvorming: DICOM, NIfTI, MINC, NRRD, tractografie en hersenoppervlakken.
integration-extra-group-cad-gis = CAD en GIS
# machine-drafted, needs native review
integration-extra-group-cad-gis-help = Plottertekeningen, technische illustraties, chiplay-outs en kaarten: HP-GL, CGM, GDSII, shapefiles, GeoPackage, KML, GPX, GeoJSON en hoogtetegels. DWG opent met de gratis ODA File Converter, STEP en IGES met OpenCASCADE.
integration-extra-group-models = 3D en moleculen
integration-extra-group-models-help = 3D-modellen, gebouwmodellen, puntenwolken en moleculen: STL, OBJ, glTF, PLY, 3MF, X3D, IFC, LAS, E57, PDB, CIF en meer.
integration-extra-group-videos = Video’s
# machine-drafted, needs native review
integration-extra-group-videos-help = Video’s: MP4, MOV, M4V, 3GP, 3G2, MKV, WebM, AVCHD (MTS, M2TS), OGV, IVF en MXF. Blackmagic RAW opent met de gratis Blackmagic RAW SDK.
integration-extra-group-other-images = SVG en andere afbeeldingen
# machine-drafted, needs native review
integration-extra-group-other-images-help = Afbeeldingen en documenten die meestal aan andere programma’s worden overgelaten: SVG-tekeningen, PDF, XPS, DjVu, EPS, WMF en EMF, Visio, OpenDocument, WordPerfect Graphics, EPUB-boeken en lettertypen, en animaties zoals MNG, FLIC en geanimeerde cursors.
integration-extra-group-text = Tekst, tabellen en ondertitels
integration-extra-group-text-help = Platte tekst en logbestanden, NFO, Markdown, CSV- en TSV-tabellen, RTF en ondertitelbestanden: SRT, WebVTT, ASS, SSA, MicroDVD, PGS, LRC, TTML en SCC.
