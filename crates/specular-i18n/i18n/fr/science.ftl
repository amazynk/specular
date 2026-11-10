# Specular — Scientific images (FITS, XISF): stretch, limits, planes, channels, coordinates. Français.

keys-mode-science = Images scientifiques
cheatsheet-science = Images scientifiques
cheatsheet-science-look = Étirement et limites
cheatsheet-science-planes = Plans et canaux
# machine-drafted, needs native review
cheatsheet-science-channels-1-9 = Afficher ou masquer les canaux 1 à 9
cheatsheet-science-channels-10-16 = Afficher ou masquer les canaux 10 à 16
palette-science-stretch = Étirement suivant
palette-science-limits = Limites suivantes
palette-science-next-plane = Plan suivant
palette-science-prev-plane = Plan précédent
palette-science-next-axis = Parcourir un autre axe
palette-science-composite = Composer les canaux en couleur
palette-science-channel-1 = Afficher ou masquer le canal 1
palette-science-channel-2 = Afficher ou masquer le canal 2
palette-science-channel-3 = Afficher ou masquer le canal 3
palette-science-channel-4 = Afficher ou masquer le canal 4
palette-science-channel-5 = Afficher ou masquer le canal 5
palette-science-channel-6 = Afficher ou masquer le canal 6
palette-science-channel-7 = Afficher ou masquer le canal 7
palette-science-channel-8 = Afficher ou masquer le canal 8
palette-science-channel-9 = Afficher ou masquer le canal 9
# machine-drafted, needs native review
palette-science-channel-10 = Afficher ou masquer le canal 10
palette-science-channel-11 = Afficher ou masquer le canal 11
palette-science-channel-12 = Afficher ou masquer le canal 12
palette-science-channel-13 = Afficher ou masquer le canal 13
palette-science-channel-14 = Afficher ou masquer le canal 14
palette-science-channel-15 = Afficher ou masquer le canal 15
palette-science-channel-16 = Afficher ou masquer le canal 16
palette-science-grid = Afficher ou masquer la grille de coordonnées
palette-science-reset = Rendu automatique
palette-science-level = Niveler les cartes de hauteur (soustraire le plan moyen)
palette-science-wiggle = Traces sismiques en wiggle à surface variable, par-dessus les couleurs
# machine-drafted, needs native review
palette-science-wireframe = Arêtes du maillage non structuré (fil de fer), par-dessus les couleurs
palette-science-import-lut = Table de couleurs depuis un fichier (ImageJ .lut)…
palette-science-hillshade = Ombrer le relief des modèles d’élévation (ombrage)
palette-science-fft = Spectre de puissance (FFT) du plan
palette-science-catalogue = Superposer un catalogue de sources (VOTable, table FITS, CSV)…
palette-science-clear-catalogues = Retirer les catalogues superposés
# A catalogue overlaid on a sky image: its name (the table's or the file's), how many of its
# sources fall on the image, and how many of those are drawn (at most a set number).
science-catalogue-toast = { $name } : { $count ->
        [one] une source sur l’image
       *[other] { $count } sources sur l’image
    }
science-catalogue-capped = { $name } : { $shown } sources affichées sur les { $count } de l’image
science-catalogue-none = { $name } : aucune source sur l’image
science-catalogue-failed = Pas un catalogue avec des positions célestes : { $name }
science-catalogues-cleared = Catalogues retirés
# The file dialog's filter name for catalogue files.
science-catalogue-filter = Catalogues de sources
# A GW sky map's credible region: the smallest area holding this share of the probability,
# drawn as a contour labelled e.g. "90 %".
science-credible = { PERCENT($percent) }
palette-science-ion-image = Image ionique à une valeur m/z (spectrométrie de masse)…
disabled-not-science = Uniquement pour les images scientifiques (astronomie, microscopie électronique et à sonde locale)
disabled-one-plane = L’image n’a qu’un seul plan
disabled-one-axis = L’image n’a pas d’autre axe à parcourir
disabled-no-channels = L’image n’a pas ce canal à composer
disabled-no-coordinates = L’image n’a pas de coordonnées célestes
disabled-not-height-map = Uniquement pour les cartes de hauteur (Gwyddion, Igor)
disabled-not-elevation = Uniquement pour les modèles d’élévation géoréférencés à une bande
disabled-not-seismic = Uniquement pour les données sismiques (SEG-Y, Seismic Unix)
disabled-not-mass-spec = Uniquement pour les images de spectrométrie de masse (imzML)
# machine-drafted, needs native review
disabled-not-ugrid = Uniquement pour les maillages non structurés (NetCDF UGRID, ICON et MPAS)
disabled-not-bayer = Uniquement pour les images brutes de caméra couleur (FITS avec matrice de Bayer, vidéo SER)
palette-debayer = Afficher les images de caméra couleur dématricées ou brutes
palette-bayer-pattern = Matrice de Bayer suivante pour ce fichier
science-debayer-on = Dématricé ({ $pattern })
science-debayer-off = Matrice de filtres couleur brute
science-bayer-pattern = Matrice de Bayer { $pattern }
# machine-drafted, needs native review
palette-annotations = Afficher ou masquer les annotations
disabled-no-annotations = L’image n’a pas d’annotations à côté d’elle (NDPA, Aperio ou ASAP XML)
science-annotations-on = Annotations affichées
science-annotations-off = Annotations masquées
science-stretch-auto = Automatique (par couleur)
# Stretches: how values between the limits map to grey levels.
science-stretch-linear = Linéaire
science-stretch-sqrt = Racine carrée
science-stretch-log = Logarithmique
science-stretch-asinh = Asinh
science-stretch-histeq = Égalisation d’histogramme
# Limits: which values show black and white. $percent is e.g. 99.5.
science-limits-zscale = Zscale
science-limits-minmax = Min/max
science-limits-percentile = { PERCENT($percent) }
# Seismic gain: limits symmetric about zero at the $percent percentile of the amplitudes.
science-limits-symmetric = ±{ PERCENT($percent) }
# The display range the file stores (ImageJ's min and max), used until other limits are chosen.
science-limits-file = Plage du fichier
# Toasts when the look changes.
science-stretch-toast = Étirement : { $stretch }
science-limits-toast = Limites : { $limits }
science-reset-toast = Rendu automatique
# An imported colour table: $name is its file name.
science-lut-toast = Table de couleurs { $name }
science-lut-failed = Pas une table de couleurs : { $name }
# The file dialog's filter name for .lut files.
science-lut-filter = Tables de couleurs ImageJ
science-composite-on = Canaux composés en couleur
science-composite-off = Un seul canal en niveaux de gris
science-channel-on = { $channel } affiché
science-channel-off = { $channel } masqué
# machine-drafted, needs native review
# A channel without a name in the file, in the shown/hidden toasts.
science-channel-unnamed = Canal { $number }
science-grid-on = Grille de coordonnées affichée
science-grid-off = Grille de coordonnées masquée
# X-ray diffraction frames: the same action shows circles of equal resolution (d-spacing).
science-rings-on = Anneaux de résolution affichés
science-rings-off = Anneaux de résolution masqués
# A resolution ring's label: its d-spacing in ångström, e.g. "2.5 Å".
science-ring = { $d } Å
# Seismic wiggle traces: drawn once the traces are at least 4 screen pixels apart.
science-wiggle-on = Traces wiggle activées (zoomer pour les voir)
science-wiggle-off = Traces wiggle désactivées
# machine-drafted, needs native review
# Unstructured meshes: the edges drawn over the faces; too many to draw in view until the
# user zooms in.
science-wireframe-on = Arêtes du maillage activées
science-wireframe-off = Arêtes du maillage désactivées
science-wireframe-dense = Arêtes du maillage activées (zoomez pour les voir)
# Plane levelling (scanning probe height maps): the best-fit plane is subtracted for display.
science-level-on = Nivellement du plan activé
science-level-off = Nivellement du plan désactivé
# Relief shading of an elevation model. $azimuth is the sun's direction in degrees clockwise
# from north, $altitude its height above the horizon in degrees.
science-hillshade-on = Ombrage activé : soleil à { $azimuth }°, { $altitude }° de hauteur
science-hillshade-off = Ombrage désactivé
# The power spectrum view: the plane's centred 2-D Fourier power, log scaled.
science-fft-on = Spectre de puissance affiché
science-fft-off = Spectre de puissance masqué
science-axis-toast = Parcours : { $axis }
# Readout lines (bottom corner).
# The value under the pointer, with the values' unit when the file states one.
science-value = Valeur { $value }
science-value-unit = Valeur { $value } { $unit }
# machine-drafted, needs native review
science-value-flags = Valeur { $value } (bits { $bits })
# machine-drafted, needs native review
# A class value under the pointer and its name, e.g. "Value 4 · Vegetation".
science-value-class = Valeur { $value } · { $class }
# machine-drafted, needs native review
# Sentinel-2 scene classes (L2A SCL, ESA's names) and L1C cloud masks.
s2-class-no-data = Aucune donnée
s2-class-defective = Saturé ou défectueux
s2-class-dark = Pixels de zones sombres
s2-class-cloud-shadow = Ombres de nuages
s2-class-vegetation = Végétation
s2-class-not-vegetated = Sans végétation
s2-class-water = Eau
s2-class-unclassified = Non classé
s2-class-cloud-medium = Nuages, probabilité moyenne
s2-class-cloud-high = Nuages, probabilité élevée
s2-class-cirrus = Cirrus fins
s2-class-snow = Neige ou glace
s2-class-clear = Dégagé
s2-class-opaque-cloud = Nuages opaques
# Several channels under the pointer: "R 12.5  G 3.1  B 0.4".
science-values = { $values }
# Sky position under the pointer, sexagesimal then degrees.
science-radec = AD { $ra }  Déc { $dec }
science-radec-deg = { $ra }°  { $dec }°
# Galactic or ecliptic longitude and latitude.
science-lonlat = l { $lon }°  b { $lat }°
science-wcs-unsupported = WCS non pris en charge : { $reason }
# machine-drafted, needs native review
# A B1950 (FK4) image: what is converted between its frame and another. Its own position
# stays B1950 in the readout. ICRS, FK4 and SAMP are names and stay as they are.
science-fk4-markers = Repères du catalogue : ICRS → FK4 converti
science-fk4-markers-galactic = Repères du catalogue : galactique → FK4 converti
science-fk4-samp = SAMP : FK4 → ICRS converti
# The look: stretch and limits.
science-look = { $stretch } · { $limits }
# Shown under the look while plane levelling is on.
science-levelled = Plan nivelé
# Shown under the look while the hillshade is on.
science-hillshaded = Ombrage : soleil à { $azimuth }°, { $altitude }° de hauteur
# Shown in the readout while the power spectrum is shown.
science-fft = Spectre de puissance (log₁₀)
# The spectrum's value under the pointer: log₁₀ of the power.
science-fft-power = log₁₀ puissance { $value }
# Spatial frequency under the pointer along x and y, and its unit ("1/µm", "1/px").
science-fft-frequency = Fréquence u { $u }  v { $v } { $unit }
# The period of that frequency: "Period 2.5 nm".
science-fft-period = Période { $period }
# Crystal orientation under the pointer (EBSD): Bunge Euler angles in degrees.
science-euler = φ₁ { $phi1 }  Φ { $phi }  φ₂ { $phi2 }
# The crystal phase under the pointer and its symmetry ("Iron fcc (m-3m)").
science-phase = Phase { $phase } ({ $symmetry })
# Title of the spectrum plot of the pixel under the pointer (an EDS map).
science-spectrum-title = Spectre en { $x }, { $y }
# Physical position under the pointer (microscopy): "x 1.25 µm  y 3.5 µm".
science-position = x { $x }  y { $y }
# machine-drafted, needs native review
# CERN ROOT histograms: the bin under the pointer on one axis. $axis is the axis title
# ("p_{T}", or "x"), $range the bin's label or its edges ("20 GeV – 25 GeV"), $bin its
# number counted from 1.
science-root-bin = { $axis } { $range } (bin { $bin })
# A ROOT graph's point under the pointer.
science-root-point = x { $x }  y { $y }
# Under- and overflow bins (fills outside the axis ranges) hold something but are not shown.
science-root-flow = { $count ->
        [one] { $count } bin de dépassement (underflow/overflow) masqué (somme { $sum })
        [many] { $count } bins de dépassement (underflow/overflow) masqués (somme { $sum })
       *[other] { $count } bins de dépassement (underflow/overflow) masqués (somme { $sum })
    }
# Bins of different widths are drawn on an even grid of pixels.
science-root-resampled = Bins de largeur variable rééchantillonnés sur une grille régulière
# machine-drafted, needs native review
# A flow cytometry plot (FCS) under the pointer: each axis's parameter and its value.
science-cytometry-at = { $x } { $xv }  { $y } { $yv }
# A cytometry histogram under the pointer: its parameter and the value.
science-cytometry-at-x = { $x } { $xv }
# Events in the plot's bin (or the histogram's bar) under the pointer.
science-cytometry-events = { $count ->
    [one] { $count } événement
    [many] { $count } d’événements
   *[other] { $count } événements
}
# The plot's axis scales ($x, $y: Linear, Log, Logicle or Arcsinh).
science-cytometry-scales = x { $x }, y { $y }
science-cytometry-scale-x = x { $x }
# Spillover compensation is applied, from the file's keyword ($keyword, e.g. $SPILLOVER).
science-cytometry-compensated = Compensé ({ $keyword })
science-cytometry-uncompensated = Non compensé
# X-ray diffraction frames: what lies under the pointer. q is the momentum transfer
# (per ångström), d the lattice spacing (ångström), 2θ the scattering angle. The symbols
# stay as they are: "q 0.512 Å⁻¹  d 12.27 Å  2θ 4.56°".
science-scatter = q { $q } Å⁻¹  d { $d } Å  2θ { $tth }°
# Seismic readout: the trace under the pointer in a 3-D volume ($inline, $crossline the
# header numbers) or a 2-D line; $trace counts from 1 in file order, $cdp is the CDP number.
science-seismic-cell = Inline { $inline }  Crossline { $crossline }
science-seismic-trace = Trace { $trace }  CDP { $cdp }
# A cell of a 3-D volume with no trace in the file.
science-seismic-gap = Aucune trace
# Two-way time (ms) or depth of the row under the pointer.
science-seismic-time = Temps { $time } ms
science-seismic-depth = Profondeur { $depth } { $unit }
# The trace's CDP (or source) X and Y with the coordinate scalar applied.
science-seismic-xy = X { $x }  Y { $y }
# machine-drafted, needs native review
# Unstructured mesh readout: the face (cell) under the pointer, counting from 0 as in the
# file's connectivity, and the node nearest to the pointer when the values sit on nodes.
science-mesh-face = Face { $face }
science-mesh-face-node = Face { $face }  Nœud { $node }
# The pointer lies between the mesh's faces (land in an ocean mesh, outside its boundary).
science-mesh-outside = Hors du maillage
# Position on an axis past the image: "Plane 3 of 40", or with its world value.
science-plane = { $axis } { $index } sur { $count }
science-plane-world = { $axis } { $index } sur { $count } ({ $world })
science-axis-unnamed = Plan
science-axis-extension = Image
science-axis-channel = Canal
# A column of a HEALPix sky map (probability, distance…) shown as the image.
science-axis-field = Champ
# X-ray event lists binned into an image: the bin size, in the event list's sky pixels
# ("Bin 2 of 4 (8 px)"), and the energy band of the events counted.
science-axis-bin = Bin
science-bin-size = { $size } px
science-axis-band = Énergie
science-band-all = toutes les énergies
# machine-drafted, needs native review
# Complex-valued arrays (NumPy, MATLAB, Zarr, TDMS): which part of each value is shown.
science-axis-part = Partie
science-part-magnitude = module
science-part-real = partie réelle
science-part-imaginary = partie imaginaire
science-part-phase = phase (radians)
# machine-drafted, needs native review
# Complex radar (SAR) images: intensity in decibels.
science-part-db = intensité (dB)
# machine-drafted, needs native review
# The frames of a thermal sequence (FLIR SEQ/CSQ), in time.
science-axis-frame = Trame
science-readout = Valeur et coordonnées
# Accessible names of the plane sliders.
science-sliders = Plans
# The scale bar's length.
science-scale-arcsec = { $value }″
science-scale-arcmin = { $value }′
science-scale-deg = { $value }°
# A physical length, e.g. "20 nm" (electron microscopy images).
science-scale-length = { $value } { $unit }
# machine-drafted, needs native review
# A length along a named axis of a plot, e.g. "20 GeV (p_{T})".
science-scale-axis = { $length } ({ $axis })
science-scale-a11y = Barre d’échelle : { $length }
# Mass spectrometry images (imzML). The image is the total ion current (each spot's summed
# intensities) until an m/z is typed; then that ion's image, its intensities summed over
# m/z ± the tolerance. $window is e.g. "760.5851 ± 5 ppm".
science-ion-tic = Courant ionique total
science-ion-window = Image ionique m/z { $window }
# The m/z prompt: $text is what is typed so far (opens on the base peak under the pointer).
science-ion-prompt = Image ionique à m/z : { $text }▏
# $tolerance is e.g. "10 ppm" or "0.1 Da"; a tolerance can also be typed after the m/z
# ("760.585 5 ppm"). Tab steps the tolerance; Enter on an empty prompt shows the total ion
# current again.
science-ion-prompt-hint = ± { $tolerance } · Tab tolérance · Entrée afficher · Échap annuler
science-ion-bad = Valeur m/z non valide : { $text }
# The spot under the pointer as the file numbers it (from 1).
science-ms-spot = Point x { $x }  y { $y }
science-ms-spot-z = Point x { $x }  y { $y }  z { $z }
science-ms-no-spectrum = Aucun spectre en ce point
# machine-drafted, needs native review
palette-science-temperature-unit = Unité de température (°C, °F, K)
palette-science-isotherm = Isotherme au-dessus ou au-dessous de la température sous le pointeur
palette-science-thermal-parameters = Émissivité et température réfléchie…
palette-science-export-temperatures = Exporter les températures (TIFF flottant ou CSV)…
palette-thermal-visible = Photo visible d’une image thermique
disabled-not-thermal = Uniquement pour les images thermiques
disabled-not-radiometric = Le fichier stocke des températures : ses paramètres ne peuvent pas être modifiés
disabled-no-visible-photo = L’image thermique n’a pas de photo visible
science-temperature-unit-toast = Températures en { $unit }
science-isotherm-above = Isotherme au-dessus de { $level }
science-isotherm-below = Isotherme au-dessous de { $level }
science-isotherm-off = Isotherme désactivée
science-thermal-emissivity-prompt = Émissivité : { $text }
science-thermal-reflected-prompt = Température réfléchie ({ $unit }) : { $text }
science-thermal-prompt-hint = Tab champ suivant · Entrée recalculer · Échap annuler
science-thermal-bad = Valeur non valide : { $text }
science-thermal-applied = Recalculé pour une émissivité de { $emissivity }, réfléchie { $reflected }
science-thermal-conditions = Émissivité { $emissivity } · réfléchie { $reflected }
science-thermal-export-tiff = TIFF flottant 32 bits
science-thermal-export-csv = CSV
science-thermal-export-done = Températures enregistrées dans { $name }
science-thermal-export-failed = Impossible d’enregistrer les températures : { $reason }
# machine-drafted, needs native review
science-values-failed = Impossible d’afficher les valeurs : { $reason }
# machine-drafted, needs native review
# Thermal sequences (FLIR SEQ/CSQ): playback and each measurement over time.
palette-science-thermal-play = Lire ou mettre en pause la séquence thermique
palette-science-thermal-series-csv = Enregistrer les températures dans le temps (CSV)…
disabled-not-thermal-sequence = Uniquement pour les séquences thermiques
thermal-seq-title = Températures dans le temps
# $time is seconds from the first frame, e.g. "0.12 s".
thermal-seq-frame = Trame { $frame } sur { $frames } · { $time }
thermal-seq-play = Lire
thermal-seq-pause = Pause
thermal-seq-playing = Lecture
thermal-seq-paused = En pause
thermal-seq-save-csv = Enregistrer en CSV…
thermal-seq-hint = Ajoutez un point, une ligne ou une zone (M) pour suivre sa courbe dans le temps.
thermal-seq-reading = Lecture de la trame { $done } sur { $total }…
thermal-seq-failed = Impossible de lire les trames : { $reason }
thermal-seq-no-series = Rien à enregistrer pour l’instant : ajoutez un point, une ligne ou une zone.
# For screen readers: each measurement's value at the frame shown, e.g.
# "1 Spot: 36.4 °C, 2 Rectangle: 30.1 °C".
thermal-seq-chart = Graphique : { $values }
# machine-drafted, needs native review
# The report (PDF) of the image shown: a thermal report for a thermal image, an inspection report for any other.
palette-thermal-report = Rapport (PDF)…
thermal-report-heading = Rapport thermique
thermal-report-default-title = Rapport d’inspection thermographique
thermal-report-title-label = Titre
thermal-report-notes-label = Notes
thermal-report-photo = Inclure la photo visible
thermal-report-folder = Ajouter les autres images thermiques de ce dossier
thermal-report-save = Enregistrer le PDF…
thermal-report-busy = Rédaction du rapport…
thermal-report-nothing = Les températures ne sont pas encore affichées.
thermal-report-saved = Rapport enregistré dans { $name } ({ $count ->
    [one] { $count } image
    [many] { $count } images
   *[other] { $count } images
})
thermal-report-failed = Impossible d’enregistrer le rapport : { $reason }
# In the PDF.
thermal-report-notes = Notes
thermal-report-measurements = Mesures
thermal-report-no-measurements = Aucune mesure.
thermal-report-thermal-caption = Image thermique
thermal-report-visible-caption = Photo visible
thermal-report-col-measurement = Mesure
thermal-report-col-min = Min
thermal-report-col-max = Max
thermal-report-col-mean = Moyenne
thermal-report-col-emissivity = Émissivité
thermal-report-whole-image = Image entière
thermal-report-camera = Caméra
thermal-report-lens = Objectif
thermal-report-date = Prise le
thermal-report-place = Lieu
thermal-report-frame = Trame
thermal-report-emissivity = Émissivité
thermal-report-reflected = Température réfléchie
thermal-report-distance = Distance
thermal-report-humidity = Humidité
thermal-report-range = Plage
thermal-report-format = Format
thermal-report-page = Page { $page } sur { $pages }
