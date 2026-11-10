# Specular — Scientific images (FITS, XISF): stretch, limits, planes, channels, coordinates. Deutsch.

keys-mode-science = Wissenschaftliche Bilder
cheatsheet-science = Wissenschaftliche Bilder
cheatsheet-science-look = Streckung und Grenzwerte
cheatsheet-science-planes = Ebenen und Kanäle
# machine-drafted, needs native review
cheatsheet-science-channels-1-9 = Kanäle 1 bis 9 ein- oder ausblenden
cheatsheet-science-channels-10-16 = Kanäle 10 bis 16 ein- oder ausblenden
palette-science-stretch = Nächste Streckung
palette-science-limits = Nächste Grenzwerte
palette-science-next-plane = Nächste Ebene
palette-science-prev-plane = Vorherige Ebene
palette-science-next-axis = Andere Achse durchlaufen
palette-science-composite = Kanäle farbig kombinieren
palette-science-channel-1 = Kanal 1 ein- oder ausblenden
palette-science-channel-2 = Kanal 2 ein- oder ausblenden
palette-science-channel-3 = Kanal 3 ein- oder ausblenden
palette-science-channel-4 = Kanal 4 ein- oder ausblenden
palette-science-channel-5 = Kanal 5 ein- oder ausblenden
palette-science-channel-6 = Kanal 6 ein- oder ausblenden
palette-science-channel-7 = Kanal 7 ein- oder ausblenden
palette-science-channel-8 = Kanal 8 ein- oder ausblenden
palette-science-channel-9 = Kanal 9 ein- oder ausblenden
# machine-drafted, needs native review
palette-science-channel-10 = Kanal 10 ein- oder ausblenden
palette-science-channel-11 = Kanal 11 ein- oder ausblenden
palette-science-channel-12 = Kanal 12 ein- oder ausblenden
palette-science-channel-13 = Kanal 13 ein- oder ausblenden
palette-science-channel-14 = Kanal 14 ein- oder ausblenden
palette-science-channel-15 = Kanal 15 ein- oder ausblenden
palette-science-channel-16 = Kanal 16 ein- oder ausblenden
palette-science-grid = Koordinatengitter ein- oder ausblenden
palette-science-reset = Automatische Darstellung
palette-science-level = Höhenkarten ausrichten (mittlere Ebene abziehen)
palette-science-wiggle = Seismische Wiggle-Spuren mit variabler Fläche über den Farben
# machine-drafted, needs native review
palette-science-wireframe = Kanten unstrukturierter Gitter (Drahtgitter) über den Farben
palette-science-import-lut = Farbtabelle aus einer Datei (ImageJ .lut)…
palette-science-hillshade = Relief von Höhenmodellen schattieren (Schummerung)
palette-science-fft = Leistungsspektrum (FFT) der Ebene
palette-science-catalogue = Quellkatalog überlagern (VOTable, FITS-Tabelle, CSV)…
palette-science-clear-catalogues = Überlagerte Kataloge entfernen
# A catalogue overlaid on a sky image: its name (the table's or the file's), how many of its
# sources fall on the image, and how many of those are drawn (at most a set number).
science-catalogue-toast = { $name }: { $count ->
        [one] eine Quelle im Bild
       *[other] { $count } Quellen im Bild
    }
science-catalogue-capped = { $name }: { $shown } von { $count } Quellen im Bild angezeigt
science-catalogue-none = { $name }: keine Quellen im Bild
science-catalogue-failed = Kein Katalog mit Himmelspositionen: { $name }
science-catalogues-cleared = Kataloge entfernt
# The file dialog's filter name for catalogue files.
science-catalogue-filter = Quellkataloge
# A GW sky map's credible region: the smallest area holding this share of the probability,
# drawn as a contour labelled e.g. "90 %".
science-credible = { PERCENT($percent) }
palette-science-ion-image = Ionenbild bei einem m/z (Massenspektrometrie)…
disabled-not-science = Nur für wissenschaftliche Bilder (Astronomie, Elektronen- und Rastersondenmikroskopie)
disabled-one-plane = Das Bild hat nur eine Ebene
disabled-one-axis = Das Bild hat keine weitere Achse zum Durchlaufen
disabled-no-channels = Das Bild hat keinen solchen Kanal zum Kombinieren
disabled-no-coordinates = Das Bild hat keine Himmelskoordinaten
disabled-not-height-map = Nur für Höhenkarten (Gwyddion, Igor)
disabled-not-elevation = Nur für georeferenzierte Höhenmodelle mit einem Band
disabled-not-seismic = Nur für Seismikdaten (SEG-Y, Seismic Unix)
disabled-not-mass-spec = Nur für Massenspektrometrie-Bilder (imzML)
# machine-drafted, needs native review
disabled-not-ugrid = Nur für unstrukturierte Gitter (UGRID-, ICON- und MPAS-NetCDF)
disabled-not-bayer = Nur für Farbkamera-Rohbilder (FITS mit Bayer-Muster, SER-Video)
palette-debayer = Farbkamerabilder entmosaikt oder roh zeigen
palette-bayer-pattern = Nächstes Bayer-Muster für diese Datei
science-debayer-on = Entmosaikt ({ $pattern })
science-debayer-off = Rohes Farbfilter-Array
science-bayer-pattern = Bayer-Muster { $pattern }
# machine-drafted, needs native review
palette-annotations = Annotationen ein- oder ausblenden
disabled-no-annotations = Neben dem Bild liegen keine Annotationen (NDPA, Aperio oder ASAP XML)
science-annotations-on = Annotationen eingeblendet
science-annotations-off = Annotationen ausgeblendet
science-stretch-auto = Automatisch (je Farbe)
# Stretches: how values between the limits map to grey levels.
science-stretch-linear = Linear
science-stretch-sqrt = Quadratwurzel
science-stretch-log = Logarithmisch
science-stretch-asinh = Asinh
science-stretch-histeq = Histogrammausgleich
# Limits: which values show black and white. $percent is e.g. 99.5.
science-limits-zscale = Zscale
science-limits-minmax = Min/Max
science-limits-percentile = { PERCENT($percent) }
# Seismic gain: limits symmetric about zero at the $percent percentile of the amplitudes.
science-limits-symmetric = ±{ PERCENT($percent) }
# The display range the file stores (ImageJ's min and max), used until other limits are chosen.
science-limits-file = Bereich aus der Datei
# Toasts when the look changes.
science-stretch-toast = Streckung: { $stretch }
science-limits-toast = Grenzwerte: { $limits }
science-reset-toast = Automatische Darstellung
# An imported colour table: $name is its file name.
science-lut-toast = Farbtabelle { $name }
science-lut-failed = Keine Farbtabelle: { $name }
# The file dialog's filter name for .lut files.
science-lut-filter = ImageJ-Farbtabellen
science-composite-on = Kanäle farbig kombiniert
science-composite-off = Ein Kanal in Graustufen
science-channel-on = { $channel } eingeblendet
science-channel-off = { $channel } ausgeblendet
# machine-drafted, needs native review
# A channel without a name in the file, in the shown/hidden toasts.
science-channel-unnamed = Kanal { $number }
science-grid-on = Koordinatengitter eingeblendet
science-grid-off = Koordinatengitter ausgeblendet
# X-ray diffraction frames: the same action shows circles of equal resolution (d-spacing).
science-rings-on = Auflösungsringe eingeblendet
science-rings-off = Auflösungsringe ausgeblendet
# A resolution ring's label: its d-spacing in ångström, e.g. "2.5 Å".
science-ring = { $d } Å
# Seismic wiggle traces: drawn once the traces are at least 4 screen pixels apart.
science-wiggle-on = Wiggle-Spuren ein (zum Sehen hineinzoomen)
science-wiggle-off = Wiggle-Spuren aus
# machine-drafted, needs native review
# Unstructured meshes: the edges drawn over the faces; too many to draw in view until the
# user zooms in.
science-wireframe-on = Gitterkanten an
science-wireframe-off = Gitterkanten aus
science-wireframe-dense = Gitterkanten an (zum Anzeigen hineinzoomen)
# Plane levelling (scanning probe height maps): the best-fit plane is subtracted for display.
science-level-on = Ebenenausgleich ein
science-level-off = Ebenenausgleich aus
# Relief shading of an elevation model. $azimuth is the sun's direction in degrees clockwise
# from north, $altitude its height above the horizon in degrees.
science-hillshade-on = Schummerung ein: Sonne bei { $azimuth }°, { $altitude }° hoch
science-hillshade-off = Schummerung aus
# The power spectrum view: the plane's centred 2-D Fourier power, log scaled.
science-fft-on = Leistungsspektrum eingeblendet
science-fft-off = Leistungsspektrum ausgeblendet
science-axis-toast = Durchlaufen: { $axis }
# Readout lines (bottom corner).
# The value under the pointer, with the values' unit when the file states one.
science-value = Wert { $value }
science-value-unit = Wert { $value } { $unit }
# machine-drafted, needs native review
science-value-flags = Wert { $value } (Bits { $bits })
# machine-drafted, needs native review
# A class value under the pointer and its name, e.g. "Value 4 · Vegetation".
science-value-class = Wert { $value } · { $class }
# machine-drafted, needs native review
# Sentinel-2 scene classes (L2A SCL, ESA's names) and L1C cloud masks.
s2-class-no-data = Keine Daten
s2-class-defective = Gesättigt oder defekt
s2-class-dark = Pixel dunkler Flächen
s2-class-cloud-shadow = Wolkenschatten
s2-class-vegetation = Vegetation
s2-class-not-vegetated = Ohne Vegetation
s2-class-water = Wasser
s2-class-unclassified = Nicht klassifiziert
s2-class-cloud-medium = Wolken, mittlere Wahrscheinlichkeit
s2-class-cloud-high = Wolken, hohe Wahrscheinlichkeit
s2-class-cirrus = Dünne Zirren
s2-class-snow = Schnee oder Eis
s2-class-clear = Wolkenfrei
s2-class-opaque-cloud = Opake Wolken
# Several channels under the pointer: "R 12.5  G 3.1  B 0.4".
science-values = { $values }
# Sky position under the pointer, sexagesimal then degrees.
science-radec = RA { $ra }  Dek { $dec }
science-radec-deg = { $ra }°  { $dec }°
# Galactic or ecliptic longitude and latitude.
science-lonlat = l { $lon }°  b { $lat }°
science-wcs-unsupported = WCS nicht unterstützt: { $reason }
# machine-drafted, needs native review
# A B1950 (FK4) image: what is converted between its frame and another. Its own position
# stays B1950 in the readout. ICRS, FK4 and SAMP are names and stay as they are.
science-fk4-markers = Katalogmarker: ICRS → FK4 umgerechnet
science-fk4-markers-galactic = Katalogmarker: galaktisch → FK4 umgerechnet
science-fk4-samp = SAMP: FK4 → ICRS umgerechnet
# The look: stretch and limits.
science-look = { $stretch } · { $limits }
# Shown under the look while plane levelling is on.
science-levelled = Ebene ausgeglichen
# Shown under the look while the hillshade is on.
science-hillshaded = Schummerung: Sonne bei { $azimuth }°, { $altitude }° hoch
# Shown in the readout while the power spectrum is shown.
science-fft = Leistungsspektrum (log₁₀)
# The spectrum's value under the pointer: log₁₀ of the power.
science-fft-power = log₁₀ Leistung { $value }
# Spatial frequency under the pointer along x and y, and its unit ("1/µm", "1/px").
science-fft-frequency = Frequenz u { $u }  v { $v } { $unit }
# The period of that frequency: "Period 2.5 nm".
science-fft-period = Periode { $period }
# Crystal orientation under the pointer (EBSD): Bunge Euler angles in degrees.
science-euler = φ₁ { $phi1 }  Φ { $phi }  φ₂ { $phi2 }
# The crystal phase under the pointer and its symmetry ("Iron fcc (m-3m)").
science-phase = Phase { $phase } ({ $symmetry })
# Title of the spectrum plot of the pixel under the pointer (an EDS map).
science-spectrum-title = Spektrum bei { $x }, { $y }
# Physical position under the pointer (microscopy): "x 1.25 µm  y 3.5 µm".
science-position = x { $x }  y { $y }
# machine-drafted, needs native review
# CERN ROOT histograms: the bin under the pointer on one axis. $axis is the axis title
# ("p_{T}", or "x"), $range the bin's label or its edges ("20 GeV – 25 GeV"), $bin its
# number counted from 1.
science-root-bin = { $axis } { $range } (Bin { $bin })
# A ROOT graph's point under the pointer.
science-root-point = x { $x }  y { $y }
# Under- and overflow bins (fills outside the axis ranges) hold something but are not shown.
science-root-flow = { $count ->
        [one] { $count } Unter-/Überlauf-Bin ausgeblendet (Summe { $sum })
       *[other] { $count } Unter-/Überlauf-Bins ausgeblendet (Summe { $sum })
    }
# Bins of different widths are drawn on an even grid of pixels.
science-root-resampled = Bins variabler Breite auf ein gleichmäßiges Raster umgerechnet
# machine-drafted, needs native review
# A flow cytometry plot (FCS) under the pointer: each axis's parameter and its value.
science-cytometry-at = { $x } { $xv }  { $y } { $yv }
# A cytometry histogram under the pointer: its parameter and the value.
science-cytometry-at-x = { $x } { $xv }
# Events in the plot's bin (or the histogram's bar) under the pointer.
science-cytometry-events = { $count ->
    [one] { $count } Ereignis
   *[other] { $count } Ereignisse
}
# The plot's axis scales ($x, $y: Linear, Log, Logicle or Arcsinh).
science-cytometry-scales = x { $x }, y { $y }
science-cytometry-scale-x = x { $x }
# Spillover compensation is applied, from the file's keyword ($keyword, e.g. $SPILLOVER).
science-cytometry-compensated = Kompensiert ({ $keyword })
science-cytometry-uncompensated = Nicht kompensiert
# X-ray diffraction frames: what lies under the pointer. q is the momentum transfer
# (per ångström), d the lattice spacing (ångström), 2θ the scattering angle. The symbols
# stay as they are: "q 0.512 Å⁻¹  d 12.27 Å  2θ 4.56°".
science-scatter = q { $q } Å⁻¹  d { $d } Å  2θ { $tth }°
# Seismic readout: the trace under the pointer in a 3-D volume ($inline, $crossline the
# header numbers) or a 2-D line; $trace counts from 1 in file order, $cdp is the CDP number.
science-seismic-cell = Inline { $inline }  Crossline { $crossline }
science-seismic-trace = Spur { $trace }  CDP { $cdp }
# A cell of a 3-D volume with no trace in the file.
science-seismic-gap = Keine Spur
# Two-way time (ms) or depth of the row under the pointer.
science-seismic-time = Zeit { $time } ms
science-seismic-depth = Tiefe { $depth } { $unit }
# The trace's CDP (or source) X and Y with the coordinate scalar applied.
science-seismic-xy = X { $x }  Y { $y }
# machine-drafted, needs native review
# Unstructured mesh readout: the face (cell) under the pointer, counting from 0 as in the
# file's connectivity, and the node nearest to the pointer when the values sit on nodes.
science-mesh-face = Fläche { $face }
science-mesh-face-node = Fläche { $face }  Knoten { $node }
# The pointer lies between the mesh's faces (land in an ocean mesh, outside its boundary).
science-mesh-outside = Außerhalb des Gitters
# Position on an axis past the image: "Plane 3 of 40", or with its world value.
science-plane = { $axis } { $index } von { $count }
science-plane-world = { $axis } { $index } von { $count } ({ $world })
science-axis-unnamed = Ebene
science-axis-extension = Bild
science-axis-channel = Kanal
# A column of a HEALPix sky map (probability, distance…) shown as the image.
science-axis-field = Feld
# X-ray event lists binned into an image: the bin size, in the event list's sky pixels
# ("Bin 2 of 4 (8 px)"), and the energy band of the events counted.
science-axis-bin = Bin
science-bin-size = { $size } px
science-axis-band = Energie
science-band-all = alle Energien
# machine-drafted, needs native review
# Complex-valued arrays (NumPy, MATLAB, Zarr, TDMS): which part of each value is shown.
science-axis-part = Anteil
science-part-magnitude = Betrag
science-part-real = Realteil
science-part-imaginary = Imaginärteil
science-part-phase = Phase (Radiant)
# machine-drafted, needs native review
# Complex radar (SAR) images: intensity in decibels.
science-part-db = Intensität (dB)
# machine-drafted, needs native review
# The frames of a thermal sequence (FLIR SEQ/CSQ), in time.
science-axis-frame = Einzelbild
science-readout = Wert und Koordinaten
# Accessible names of the plane sliders.
science-sliders = Ebenen
# The scale bar's length.
science-scale-arcsec = { $value }″
science-scale-arcmin = { $value }′
science-scale-deg = { $value }°
# A physical length, e.g. "20 nm" (electron microscopy images).
science-scale-length = { $value } { $unit }
# machine-drafted, needs native review
# A length along a named axis of a plot, e.g. "20 GeV (p_{T})".
science-scale-axis = { $length } ({ $axis })
science-scale-a11y = Maßstabsleiste: { $length }
# Mass spectrometry images (imzML). The image is the total ion current (each spot's summed
# intensities) until an m/z is typed; then that ion's image, its intensities summed over
# m/z ± the tolerance. $window is e.g. "760.5851 ± 5 ppm".
science-ion-tic = Totalionenstrom
science-ion-window = Ionenbild m/z { $window }
# The m/z prompt: $text is what is typed so far (opens on the base peak under the pointer).
science-ion-prompt = Ionenbild bei m/z: { $text }▏
# $tolerance is e.g. "10 ppm" or "0.1 Da"; a tolerance can also be typed after the m/z
# ("760.585 5 ppm"). Tab steps the tolerance; Enter on an empty prompt shows the total ion
# current again.
science-ion-prompt-hint = ± { $tolerance } · Tab Toleranz · Eingabe anzeigen · Esc abbrechen
science-ion-bad = Kein m/z-Wert: { $text }
# The spot under the pointer as the file numbers it (from 1).
science-ms-spot = Messpunkt x { $x }  y { $y }
science-ms-spot-z = Messpunkt x { $x }  y { $y }  z { $z }
science-ms-no-spectrum = Kein Spektrum an diesem Messpunkt
# machine-drafted, needs native review
palette-science-temperature-unit = Temperatureinheit (°C, °F, K)
palette-science-isotherm = Isotherme über oder unter der Temperatur am Zeiger
palette-science-thermal-parameters = Emissionsgrad und reflektierte Temperatur…
palette-science-export-temperatures = Temperaturen exportieren (Float-TIFF oder CSV)…
palette-thermal-visible = Sichtbares Foto eines Wärmebilds
disabled-not-thermal = Nur für Wärmebilder
disabled-not-radiometric = Die Datei speichert Temperaturen: Ihre Parameter lassen sich nicht ändern
disabled-no-visible-photo = Das Wärmebild hat kein sichtbares Foto
science-temperature-unit-toast = Temperaturen in { $unit }
science-isotherm-above = Isotherme über { $level }
science-isotherm-below = Isotherme unter { $level }
science-isotherm-off = Isotherme aus
science-thermal-emissivity-prompt = Emissionsgrad: { $text }
science-thermal-reflected-prompt = Reflektierte Temperatur ({ $unit }): { $text }
science-thermal-prompt-hint = Tab nächstes Feld · Eingabe neu berechnen · Esc abbrechen
science-thermal-bad = Kein gültiger Wert: { $text }
science-thermal-applied = Neu berechnet für Emissionsgrad { $emissivity }, reflektiert { $reflected }
science-thermal-conditions = Emissionsgrad { $emissivity } · reflektiert { $reflected }
science-thermal-export-tiff = 32-Bit-Float-TIFF
science-thermal-export-csv = CSV
science-thermal-export-done = Temperaturen in { $name } gespeichert
science-thermal-export-failed = Temperaturen konnten nicht gespeichert werden: { $reason }
# machine-drafted, needs native review
science-values-failed = Die Werte konnten nicht angezeigt werden: { $reason }
# machine-drafted, needs native review
# Thermal sequences (FLIR SEQ/CSQ): playback and each measurement over time.
palette-science-thermal-play = Wärmebildsequenz abspielen oder anhalten
palette-science-thermal-series-csv = Temperaturen im Zeitverlauf speichern (CSV)…
disabled-not-thermal-sequence = Nur für Wärmebildsequenzen
thermal-seq-title = Temperaturen im Zeitverlauf
# $time is seconds from the first frame, e.g. "0.12 s".
thermal-seq-frame = Einzelbild { $frame } von { $frames } · { $time }
thermal-seq-play = Abspielen
thermal-seq-pause = Pause
thermal-seq-playing = Wird abgespielt
thermal-seq-paused = Angehalten
thermal-seq-save-csv = CSV speichern…
thermal-seq-hint = Einen Messpunkt, eine Linie oder einen Bereich (M) hinzufügen, um den Verlauf zu zeichnen.
thermal-seq-reading = Einzelbild { $done } von { $total } wird gelesen…
thermal-seq-failed = Die Einzelbilder konnten nicht gelesen werden: { $reason }
thermal-seq-no-series = Noch nichts zu speichern: einen Messpunkt, eine Linie oder einen Bereich hinzufügen.
# For screen readers: each measurement's value at the frame shown, e.g.
# "1 Spot: 36.4 °C, 2 Rectangle: 30.1 °C".
thermal-seq-chart = Diagramm: { $values }
# machine-drafted, needs native review
# The report (PDF) of the image shown: a thermal report for a thermal image, an inspection report for any other.
palette-thermal-report = Bericht (PDF)…
thermal-report-heading = Thermografiebericht
thermal-report-default-title = Thermografischer Prüfbericht
thermal-report-title-label = Titel
thermal-report-notes-label = Notizen
thermal-report-photo = Sichtbares Foto einfügen
thermal-report-folder = Die anderen Wärmebilder in diesem Ordner hinzufügen
thermal-report-save = PDF speichern…
thermal-report-busy = Bericht wird geschrieben…
thermal-report-nothing = Die Temperaturen werden noch nicht angezeigt.
thermal-report-saved = Bericht in { $name } gespeichert ({ $count ->
    [one] { $count } Bild
   *[other] { $count } Bilder
})
thermal-report-failed = Der Bericht konnte nicht gespeichert werden: { $reason }
# In the PDF.
thermal-report-notes = Notizen
thermal-report-measurements = Messungen
thermal-report-no-measurements = Keine Messungen.
thermal-report-thermal-caption = Wärmebild
thermal-report-visible-caption = Sichtbares Foto
thermal-report-col-measurement = Messung
thermal-report-col-min = Min.
thermal-report-col-max = Max.
thermal-report-col-mean = Mittelwert
thermal-report-col-emissivity = Emissionsgrad
thermal-report-whole-image = Ganzes Bild
thermal-report-camera = Kamera
thermal-report-lens = Objektiv
thermal-report-date = Aufgenommen
thermal-report-place = Ort
thermal-report-frame = Einzelbild
thermal-report-emissivity = Emissionsgrad
thermal-report-reflected = Reflektierte Temperatur
thermal-report-distance = Entfernung
thermal-report-humidity = Luftfeuchtigkeit
thermal-report-range = Bereich
thermal-report-format = Format
thermal-report-page = Seite { $page } von { $pages }
