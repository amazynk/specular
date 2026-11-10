# machine-drafted, needs native review
# Integrazione profonda con il desktop: Impostazioni → Integrazione, imposta come sfondo. Italiano.

settings-section-integration = Integrazione
settings-integration-default-viewer = Visualizzatore di immagini predefinito
settings-integration-default-viewer-help = Apri le immagini con { -app-name } con un doppio clic. Disattivandolo si ripristina il visualizzatore usato in precedenza.
# machine-drafted, needs native review
settings-integration-extra-default-types = Altri tipi di file
settings-integration-extra-default-types-help = Altri tipi di file da aprire con { -app-name } con un doppio clic, scelti in Visualizzatore di immagini predefinito.
settings-integration-thumbnailer = Miniature nel file manager
settings-integration-thumbnailer-help = Permette ai file manager di mostrare anteprime di formati che non sanno leggere da soli, come JPEG XL, HEIC e i file RAW delle fotocamere.
settings-integration-single-instance = Istanza singola tramite D-Bus
settings-integration-single-instance-help = I file aperti dal file manager, da un launcher o dal terminale vanno alla finestra di { -app-name } già aperta. Vale per le finestre aperte in seguito.
settings-integration-file-manager-actions = Azioni nel file manager
settings-integration-file-manager-actions-help = Aggiunge «Apri», «Confronta» e «Imposta come sfondo» ai menu contestuali di Nautilus, Dolphin, Thunar e Nemo.
settings-integration-search-provider = Ricerca del desktop
# machine-drafted, needs native review
settings-integration-search-provider-help = Trova le immagini della tua libreria nella ricerca di GNOME e KDE. Le parole funzionano come nella barra di ricerca della libreria.
# machine-drafted, needs native review
settings-integration-url-scheme = Apri i link specular://
settings-integration-url-scheme-help = I link come specular:///home/me/photo.jpg in un browser o in un altro programma si aprono in { -app-name }. Vengono aperti solo file di questo computer, e quelli insoliti solo dopo la tua conferma.

integration-state-on = Attivo
integration-state-off = Disattivo
integration-state-partial = Configurato solo in parte. Applica di nuovo per ripararlo.
integration-state-unavailable = Non disponibile su questo desktop
integration-state-sandboxed = Non disponibile in una sandbox (Flatpak)
integration-state-no-bus = Non disponibile: nessuna sessione D-Bus
integration-state-no-file-manager = Non disponibile: nessun file manager supportato trovato
integration-state-no-search = Non disponibile: né GNOME Shell né KRunner trovati
integration-state-working = In corso…
integration-state-details = { $state } · { $details }
integration-detail-types = tipi di immagine: { $count } di { $total }
integration-detail-thumbnail-types = tipi di immagine: { $count }
integration-apply = Applica
integration-remove = Rimuovi
integration-done-on = «{ $option }» è attivo
integration-done-off = «{ $option }» è disattivo
integration-failed = Impossibile modificare «{ $option }»: { $reason }
# machine-drafted, needs native review
integration-search-remove-ask = Disattivare la ricerca del desktop?
integration-search-remove-detail = { -app-name } viene tolto dalla ricerca di { $desktops }. La libreria e le immagini restano.
search-provider-open-folder = Apri cartella

integration-label-open = Apri in { -app-name }
integration-label-compare = Confronta in { -app-name }
integration-label-wallpaper = Imposta come sfondo
integration-label-generic-name = Visualizzatore di immagini
integration-label-comment = Apri e sfoglia all’istante foto di grandi dimensioni

palette-set-wallpaper = Imposta come sfondo
wallpaper-set = Sfondo impostato ({ $method })
wallpaper-failed = Impossibile impostare lo sfondo: { $reason }

# machine-drafted, needs native review
url-invalid = Impossibile aprire questo link di { -app-name }: { $reason }
url-not-found = I file indicati da questo link non esistono
url-remote-refused = { -app-name } apre solo file di questo computer, non { $source }
url-confirm-files = { $more ->
    [0] «{ $file }»
   *[other] «{ $file }» e altri { $more }
}
url-confirm-title = { $reason ->
    [dots] Aprire { url-confirm-files }? Il link lo raggiunge tramite cartelle «..».
    [network] Aprire { url-confirm-files }? Si trova su una condivisione di rete o un dispositivo.
   *[hidden] Aprire { url-confirm-files }? Il nome contiene caratteri invisibili o di direzione del testo.
}
url-confirm-open = Apri

# Launcher actions and recent files: machine-drafted, needs native review
integration-label-new-window = Nuova finestra
integration-label-library = Libreria
integration-label-slideshow = Presentazione delle Immagini
settings-integration-recent-files = File recenti del desktop
settings-integration-recent-files-help = Le immagini aperte compaiono in «Recenti» nel gestore file e negli elenchi dei file recenti del desktop. Rimuovendo l'opzione, le voci di { -app-name } vengono tolte da quegli elenchi.
integration-detail-recent = file: { $count }
# machine-drafted, needs native review
integration-extra-title = Altri tipi di file
# machine-drafted, needs native review
integration-extra-intro = Oltre alle immagini, { -app-name } può aprire questi file con un doppio clic. Nulla è selezionato finché non lo selezioni tu. I programmi che modificano questi file restano in «Apri con», e deselezionando un tipo torna al programma che aveva prima. Blackmagic RAW, DWG, STEP, IGES e Zeiss CZI richiedono anche software gratuito dei loro produttori: vedi Impostazioni › Decoder opzionali.
integration-extra-chosen = { $count ->
    [0] Nessuno scelto
    [one] 1 tipo scelto
    [many] { $count } di tipi scelti
   *[other] { $count } tipi scelti
}
integration-extra-count = { $on } di { $total }
integration-extra-none-yet = Ancora nessuno
integration-extra-keywords = tipi di file formati associazioni doppio clic apri con editor scienza medicina CAD GIS 3D molecole video SVG PDF
integration-extra-group-editors = File di editor
# machine-drafted, needs native review
integration-extra-group-editors-help = Documenti di programmi di pittura e disegno: Krita, GIMP, Paint.NET, PaintShop Pro, Procreate, Affinity, OpenRaster, Illustrator, CorelDRAW, Clip Studio Paint, Sketch e Figma. Il doppio clic mostra l’immagine; la modifichi sempre nel programma che l’ha creata.
integration-extra-group-science = Scienza
integration-extra-group-science-help = File di strumenti e software di ricerca: termocamere, microscopi, misure di superficie, HDF5, NetCDF, dati sismici e meteo.
integration-extra-group-medical = Medicina
integration-extra-group-medical-help = Esami medici e neuroimmagini: DICOM, NIfTI, MINC, NRRD, trattografia e superfici cerebrali.
integration-extra-group-cad-gis = CAD e GIS
# machine-drafted, needs native review
integration-extra-group-cad-gis-help = Disegni da plotter, illustrazioni tecniche, layout di chip e mappe: HP-GL, CGM, GDSII, shapefile, GeoPackage, KML, GPX, GeoJSON e tile di elevazione. DWG si apre con ODA File Converter, gratuito, STEP e IGES con OpenCASCADE.
integration-extra-group-models = 3D e molecole
integration-extra-group-models-help = Modelli 3D, modelli di edifici, nuvole di punti e molecole: STL, OBJ, glTF, PLY, 3MF, X3D, IFC, LAS, E57, PDB, CIF e altro.
integration-extra-group-videos = Video
# machine-drafted, needs native review
integration-extra-group-videos-help = Video: MP4, MOV, M4V, 3GP, 3G2, MKV, WebM, AVCHD (MTS, M2TS), OGV, IVF e MXF. Blackmagic RAW si apre con il Blackmagic RAW SDK, gratuito.
integration-extra-group-other-images = SVG e altre immagini
# machine-drafted, needs native review
integration-extra-group-other-images-help = Immagini e documenti di solito lasciati ad altri programmi: disegni SVG, PDF, XPS, DjVu, EPS, WMF ed EMF, Visio, OpenDocument, WordPerfect Graphics, libri EPUB e font, e animazioni come MNG, FLIC e cursori animati.
integration-extra-group-text = Testo, tabelle e sottotitoli
integration-extra-group-text-help = Testo semplice e log, NFO, Markdown, tabelle CSV e TSV, RTF e file di sottotitoli: SRT, WebVTT, ASS, SSA, MicroDVD, PGS, LRC, TTML e SCC.
