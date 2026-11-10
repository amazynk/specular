# Tiefe Desktop-Integration: Einstellungen → Integration, als Hintergrundbild festlegen.

settings-section-integration = Integration
settings-integration-default-viewer = Standard-Bildbetrachter
settings-integration-default-viewer-help = Bilder per Doppelklick mit { -app-name } öffnen. Beim Entfernen wird der vorherige Betrachter wiederhergestellt.
# machine-drafted, needs native review
settings-integration-extra-default-types = Weitere Dateitypen
settings-integration-extra-default-types-help = Weitere Dateiarten, die sich per Doppelklick mit { -app-name } öffnen; Auswahl unter Standard-Bildbetrachter.
settings-integration-thumbnailer = Vorschaubilder im Dateimanager
settings-integration-thumbnailer-help = Dateimanager zeigen Vorschauen auch für Formate, die sie selbst nicht lesen können, etwa JPEG XL, HEIC und Kamera-Rohdaten.
settings-integration-single-instance = Einzelne Instanz über D-Bus
settings-integration-single-instance-help = Aus dem Dateimanager, einem Starter oder dem Terminal geöffnete Dateien gehen an das laufende { -app-name }-Fenster. Gilt für danach geöffnete Fenster.
settings-integration-file-manager-actions = Aktionen im Dateimanager
settings-integration-file-manager-actions-help = „Öffnen“, „Vergleichen“ und „Als Hintergrundbild festlegen“ zu den Kontextmenüs von Nautilus, Dolphin, Thunar und Nemo hinzufügen.
settings-integration-search-provider = Desktop-Suche
# machine-drafted, needs native review
settings-integration-search-provider-help = Bilder aus Ihrer Bibliothek in der GNOME- und KDE-Suche finden. Suchwörter wirken wie in der Suchleiste der Bibliothek.
# machine-drafted, needs native review
settings-integration-url-scheme = specular://-Links öffnen
settings-integration-url-scheme-help = Links wie specular:///home/me/photo.jpg in einem Browser oder einem anderen Programm öffnen sich in { -app-name }. Nur Dateien auf diesem Computer werden geöffnet, ungewöhnliche erst nach Ihrer Bestätigung.

integration-state-on = An
integration-state-off = Aus
integration-state-partial = Teilweise eingerichtet. Erneut anwenden, um es zu reparieren.
integration-state-unavailable = Auf diesem Desktop nicht verfügbar
integration-state-sandboxed = In einer Sandbox (Flatpak) nicht verfügbar
integration-state-no-bus = Nicht verfügbar: Es gibt keine D-Bus-Sitzung
integration-state-no-file-manager = Nicht verfügbar: Kein unterstützter Dateimanager gefunden
integration-state-no-search = Nicht verfügbar: Weder GNOME Shell noch KRunner gefunden
integration-state-working = Wird ausgeführt …
integration-state-details = { $state } · { $details }
integration-detail-types = Bildtypen: { $count } von { $total }
integration-detail-thumbnail-types = Bildtypen: { $count }
integration-apply = Anwenden
integration-remove = Entfernen
integration-done-on = „{ $option }“ ist an
integration-done-off = „{ $option }“ ist aus
integration-failed = „{ $option }“ konnte nicht geändert werden: { $reason }
# machine-drafted, needs native review
integration-search-remove-ask = Desktop-Suche ausschalten?
integration-search-remove-detail = { -app-name } wird aus der Suche von { $desktops } entfernt. Ihre Bibliothek und Bilder bleiben.
search-provider-open-folder = Ordner öffnen

integration-label-open = In { -app-name } öffnen
integration-label-compare = In { -app-name } vergleichen
integration-label-wallpaper = Als Hintergrundbild festlegen
integration-label-generic-name = Bildbetrachter
integration-label-comment = Große Fotos sofort öffnen und durchblättern

palette-set-wallpaper = Als Hintergrundbild festlegen
wallpaper-set = Hintergrundbild festgelegt ({ $method })
wallpaper-failed = Hintergrundbild konnte nicht festgelegt werden: { $reason }

# machine-drafted, needs native review
url-invalid = Dieser { -app-name }-Link kann nicht geöffnet werden: { $reason }
url-not-found = Die Dateien, die dieser Link nennt, existieren nicht
url-remote-refused = { -app-name } öffnet nur Dateien auf diesem Computer, nicht { $source }
url-confirm-files = { $more ->
    [0] „{ $file }“
   *[other] „{ $file }“ und { $more } weitere
}
url-confirm-title = { $reason ->
    [dots] { url-confirm-files } öffnen? Der Link erreicht sie über „..“-Ordner.
    [network] { url-confirm-files } öffnen? Sie liegt auf einer Netzwerkfreigabe oder einem Gerät.
   *[hidden] { url-confirm-files } öffnen? Der Name enthält unsichtbare oder Textrichtungszeichen.
}
url-confirm-open = Öffnen

# Launcher actions and recent files: machine-drafted, needs native review
integration-label-new-window = Neues Fenster
integration-label-library = Bibliothek
integration-label-slideshow = Diashow der Bilder
settings-integration-recent-files = Zuletzt verwendete Dateien im Desktop
settings-integration-recent-files-help = Geöffnete Bilder erscheinen unter „Zuletzt verwendet“ im Dateimanager und in den Listen zuletzt verwendeter Dateien des Desktops. Beim Entfernen werden die Einträge von { -app-name } wieder aus diesen Listen genommen.
integration-detail-recent = Dateien: { $count }
# machine-drafted, needs native review
integration-extra-title = Weitere Dateitypen
# machine-drafted, needs native review
integration-extra-intro = Neben Bildern kann { -app-name } auch diese Dateien per Doppelklick öffnen. Nichts ist angehakt, solange Sie es nicht selbst anhaken. Die Programme, die diese Dateien bearbeiten, bleiben unter „Öffnen mit“, und ein abgewählter Typ geht an sein vorheriges Programm zurück. Blackmagic RAW, DWG, STEP, IGES und Zeiss CZI brauchen außerdem kostenlose Software ihrer Hersteller: siehe Einstellungen › Optionale Decoder.
integration-extra-chosen = { $count ->
    [0] Keiner ausgewählt
    [one] 1 Typ ausgewählt
   *[other] { $count } Typen ausgewählt
}
integration-extra-count = { $on } von { $total }
integration-extra-none-yet = Noch keine
integration-extra-keywords = Dateitypen Formate Dateizuordnung Verknüpfung Doppelklick öffnen mit Editor Wissenschaft Medizin CAD GIS 3D Moleküle Video SVG PDF
integration-extra-group-editors = Editor-Dateien
# machine-drafted, needs native review
integration-extra-group-editors-help = Dokumente von Mal- und Zeichenprogrammen: Krita, GIMP, Paint.NET, PaintShop Pro, Procreate, Affinity, OpenRaster, Illustrator, CorelDRAW, Clip Studio Paint, Sketch und Figma. Ein Doppelklick zeigt das Bild; bearbeitet wird es weiter im Programm, das es erstellt hat.
integration-extra-group-science = Wissenschaft
integration-extra-group-science-help = Dateien von Messgeräten und Forschungssoftware: Wärmebildkameras, Mikroskope, Oberflächenmessungen, HDF5, NetCDF, Seismik- und Wetterdaten.
integration-extra-group-medical = Medizin
integration-extra-group-medical-help = Medizinische Aufnahmen und Hirnbildgebung: DICOM, NIfTI, MINC, NRRD, Traktografie und Hirnoberflächen.
integration-extra-group-cad-gis = CAD und GIS
# machine-drafted, needs native review
integration-extra-group-cad-gis-help = Plotterzeichnungen, technische Illustrationen, Chip-Layouts und Karten: HP-GL, CGM, GDSII, Shapefiles, GeoPackage, KML, GPX, GeoJSON und Höhenkacheln. DWG wird mit dem kostenlosen ODA File Converter geöffnet, STEP und IGES mit OpenCASCADE.
integration-extra-group-models = 3D und Moleküle
integration-extra-group-models-help = 3D-Modelle, Gebäudemodelle, Punktwolken und Moleküle: STL, OBJ, glTF, PLY, 3MF, X3D, IFC, LAS, E57, PDB, CIF und mehr.
integration-extra-group-videos = Videos
# machine-drafted, needs native review
integration-extra-group-videos-help = Videos: MP4, MOV, M4V, 3GP, 3G2, MKV, WebM, AVCHD (MTS, M2TS), OGV, IVF und MXF. Blackmagic RAW wird mit dem kostenlosen Blackmagic RAW SDK geöffnet.
integration-extra-group-other-images = SVG und andere Bilder
# machine-drafted, needs native review
integration-extra-group-other-images-help = Bilder und Dokumente, die meist anderen Programmen überlassen werden: SVG-Zeichnungen, PDF, XPS, DjVu, EPS, WMF und EMF, Visio, OpenDocument, WordPerfect Graphics, EPUB-Bücher und Schriften sowie Animationen wie MNG, FLIC und animierte Mauszeiger.
integration-extra-group-text = Text, Tabellen und Untertitel
integration-extra-group-text-help = Reiner Text und Protokolle, NFO, Markdown, CSV- und TSV-Tabellen, RTF sowie Untertiteldateien: SRT, WebVTT, ASS, SSA, MicroDVD, PGS, LRC, TTML und SCC.
