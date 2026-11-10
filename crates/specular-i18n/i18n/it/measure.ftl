# Specular — Strumento di misura (lunghezze, angoli, regioni e relative statistiche su qualsiasi immagine). Bozza automatica, da rivedere da un madrelingua.

palette-measure = Misura
palette-measure-copy-csv = Copia misure come CSV
palette-measure-export = Esporta misure…
toast-copied-measurements = Misure copiate come CSV

measure-tool-title = Misura
measure-tool-list = Misure
measure-tool-count = { $count ->
    [0] Nessuna misura
    [one] Una misura
   *[other] { $count } misure
}
measure-tool-empty = Scegli uno strumento e disegna sull’immagine.

measure-tool-point = Punto
measure-tool-line = Linea
measure-tool-polyline = Polilinea
measure-tool-angle = Angolo
measure-tool-rect = Rettangolo
measure-tool-ellipse = Ellisse
measure-tool-polygon = Poligono
measure-tool-freehand = A mano libera
measure-tool-calibrate = Calibra
# machine-drafted, needs native review
measure-tool-spot = Punto

measure-tool-undo = Annulla
measure-tool-delete = Elimina
measure-tool-copy = Copia CSV
measure-tool-export = Esporta…
measure-tool-import = Importa…
measure-tool-done = Fine
measure-tool-unit = Unità: { $unit }
measure-tool-apply = Applica

measure-tool-hint-line = Trascina o fai clic su due punti. Le frecce spostano il mirino, Spazio posiziona un punto.
measure-tool-hint-angle = Fai clic su tre punti; il secondo è il vertice.
measure-tool-hint-rect = Trascina o fai clic su due angoli.
measure-tool-hint-ellipse = Trascina o fai clic su due angoli del suo riquadro.
measure-tool-hint-polygon = Fai clic sugli angoli; Invio o un clic sul primo angolo lo chiude.
measure-tool-hint-calibrate = Traccia una linea su qualcosa di lunghezza nota.
# machine-drafted, needs native review
measure-tool-hint-spot = Fai clic su un punto per leggerne la temperatura.
measure-tool-known-length = Lunghezza nota: { $value } { $unit } (digita il numero, U cambia unità, Invio applica)
measure-tool-calibrated = Calibrato: un pixel misura { $size } { $unit }
measure-tool-bad-length = Digita una lunghezza maggiore di zero.

measure-tool-item = { $n }. { $shape }
measure-tool-area = Area { $value }
measure-tool-mean = Media { $mean } ± { $sd }
measure-tool-means = Media { $values }
measure-tool-range = Intervallo { $min } – { $max } su { $count } px
measure-tool-pixels = Su { $count } px
measure-tool-stats-pending = Lettura dei pixel…
measure-tool-stats-none = Nessun valore di pixel qui
measure-tool-approximate = Da una copia ridotta (approssimato)

measure-tool-deleted = Misura eliminata
measure-tool-undone = Annullato
measure-tool-nothing = Nessuna misura su questa immagine
measure-tool-exported = Misure salvate in { $name }
measure-tool-export-failed = Impossibile salvare le misure: { $reason }
measure-tool-imported = { $count ->
    [one] Una misura importata.
   *[other] { $count } misure importate.
}
measure-tool-skipped = { $count ->
    [one] Impossibile leggere una forma.
   *[other] Impossibile leggere { $count } forme.
}
measure-tool-import-failed = Impossibile leggere le misure: { $reason }
measure-tool-too-large = Il file è troppo grande.
# machine-drafted, needs native review
measure-tool-sr-not-dicom = DICOM SR richiede un’immagine DICOM. Salva invece come CSV o GeoJSON.

measure-source-uncalibrated = Pixel (non calibrato)
measure-source-dicom-pixel-spacing = Spaziatura pixel DICOM
measure-source-dicom-calibrated = Spaziatura calibrata DICOM
measure-source-dicom-imager-pixel-spacing = Spaziatura DICOM al rivelatore
measure-source-dicom-magnification = Spaziatura DICOM corretta per l’ingrandimento
measure-source-dicom-ultrasound-region = Regione ecografica DICOM
measure-source-tiff-resolution = Risoluzione TIFF
measure-source-exif-resolution = Risoluzione EXIF
measure-source-ome-physical-size = Dimensione fisica OME
measure-source-fits-wcs = Coordinate celesti FITS
measure-source-vendor-tag = Metadati del produttore
measure-source-drawing-units = Unità del disegno
# machine-drafted, needs native review
measure-source-drawing-scale = Dalla scala del disegno
# machine-drafted, needs native review
measure-source-drawing-scale-ratio = Dal disegno (scala { $scale })
# machine-drafted, needs native review
measure-source-drawing-scale-viewport = Dal disegno (scala { $scale }, vista { $viewport })
# machine-drafted, needs native review
measure-source-map-scale = Scala della mappa al centro (approssimata; gli strumenti per mappe misurano sulla Terra)
measure-source-user = La tua calibrazione
