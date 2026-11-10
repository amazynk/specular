# machine-drafted, needs native review
# Panneau de métadonnées de Specular — français (voir i18n/en/metadata.ftl pour les conventions).

## Titres des groupes du résumé

group-camera = Appareil photo
group-exposure = Exposition
group-lens = Objectif
group-date = Date et heure
group-location = Lieu
# machine-drafted, needs native review
# A DJI drone photo: the gimbal's and the aircraft's attitude, the height above take-off.
group-drone = Drone
label-gimbal = Nacelle
label-flight = Appareil
label-relative-altitude = Au-dessus du décollage
drone-attitude = lacet { $yaw }, tangage { $pitch }, roulis { $roll }
# A DJI thermal camera's radiometric JPEG: its photo opens, its temperatures can't.
label-thermal-data = Températures
drone-thermal-refused = Non affiché : la licence du SDK de DJI ne le permet pas
drone-thermal-refused-detail = Lire les températures DJI nécessite le Thermal SDK de DJI, dont la licence ne permet de l’utiliser qu’aux programmes qui le fournissent. L’image et ses métadonnées sont affichées.
group-image = Image
group-file = Fichier
group-rights = Droits et description
group-credentials = Identifiants de contenu
# machine-drafted, needs native review
group-medical = Image médicale
# machine-drafted, needs native review
group-inspection = Pièce et contrôle
group-nde-image = Image de contrôle (DICONDE)
# machine-drafted, needs native review
# Industrial CT volumes (Zeiss Xradia, Amira, Bruker SkyScan, Nikon X-Tek, VGI, raw volumes).
group-industrial-image = Image de contrôle
group-science = Image scientifique
group-em = Microscopie électronique
group-spm = Microscopie à sonde locale
group-slide = Lame
group-microscopy = Microscopie
# X-ray diffraction detector frames (CBF, EDF, SMV, Bruker, MarCCD, R-AXIS).
group-diffraction = Diffraction
group-surface = Topographie de surface
group-ebsd = Diffraction des électrons rétrodiffusés
group-seismic = Données sismiques
# machine-drafted, needs native review
group-asdf = ASDF
# machine-drafted, needs native review
# CERN ROOT files (particle physics histograms and graphs).
group-root = ROOT
# machine-drafted, needs native review
# CASA images (radio interferometry: the .image, .residual, .model directories tclean writes).
group-casa = Image CASA
# machine-drafted, needs native review
# GIFTI files: brain surfaces and the values on their vertices (Human Connectome Project
# and FreeSurfer exports).
group-gifti = GIFTI
# CIFTI-2 files: maps over cortical surfaces and subcortical voxels ("grayordinates").
group-cifti = CIFTI
# White-matter fibre tracts (TrackVis, MRtrix).
group-tractography = Tractographie
# machine-drafted, needs native review
group-annotations = Annotations
# Imaging mass spectrometry (imzML): a mass spectrum per pixel.
group-mass-spec = Imagerie par spectrométrie de masse
# machine-drafted, needs native review
group-thermal = Image thermique
# machine-drafted, needs native review
# Flow cytometry list-mode data (FCS): events of a cell sorter or analyser.
group-cytometry = Cytométrie en flux
# machine-drafted, needs native review
group-spectroscopy = Spectroscopie
# machine-drafted, needs native review
# Unstructured meshes of ocean, coastal and climate models (UGRID, ICON, MPAS in NetCDF).
group-ugrid = Maillage non structuré
group-drawing = Dessin
# machine-drafted, needs native review
group-volume = Volume

## Libellés des éléments du résumé

label-camera = Appareil photo
label-software = Logiciel
label-application = Application
label-shutter = Vitesse d’obturation
label-aperture = Ouverture
label-iso = Sensibilité ISO
label-focal-length = Focale
label-exposure-bias = Correction d’exposition
label-program = Mode
label-metering = Mode de mesure
label-white-balance = Balance des blancs
label-flash = Flash
label-lens = Objectif
label-lens-make = Fabricant de l’objectif
label-lens-range = Plage de l’objectif
# machine-drafted, needs native review
label-focus-mode = Mode de mise au point
label-stabilisation = Stabilisation d’image
label-taken = Prise de vue
label-digitised = Numérisation
label-edited = Dernière modification
label-place = Lieu
label-coordinates = Coordonnées
label-altitude = Altitude
label-direction = Direction
label-accuracy = Précision
label-gps-time = Heure GPS
label-dimensions = Dimensions
label-encoding = Encodage
# machine-drafted, needs native review
label-motion = Mouvement
motion-kind-motion-photo = Photo animée
motion-kind-micro-video = Photo animée (MicroVideo)
motion-kind-samsung = Photo animée Samsung
motion-kind-live-photo = Live Photo
motion-duration = { $seconds } s
label-colour-profile = Profil colorimétrique
label-orientation = Orientation
label-file-name = Nom
label-folder = Dossier
label-file-size = Taille
label-format = Format
label-modified = Modification
label-title = Titre
label-description = Description
label-creator = Créateur
label-copyright = Copyright
label-keywords = Mots-clés
label-rating = Note
label-colour-label = Libellé
label-credentials = Identifiants de contenu

## Groupes « Tous les champs » (les noms de balises restent ceux des normes)

fields-ifd0 = IFD0 (image principale)
fields-exif = Exif
fields-gps = GPS
fields-interop = Interopérabilité
fields-ifd1 = IFD1 (vignette)
fields-icc = En-tête du profil ICC
fields-xmp = XMP : { $namespace } ({ $prefix })
xmp-namespace = { $prefix ->
    [dc] Dublin Core
    [xmp] De base
    [xmpRights] Droits
    [xmpMM] Gestion des médias
    [xmpDM] Médias dynamiques
    [tiff] TIFF
    [exif] Exif
    [exifEX] Exif 2.3+
    [aux] Accessoires d’appareil photo
    [photoshop] Photoshop
    [crs] Paramètres Camera Raw
    [lr] Lightroom
    [Iptc4xmpCore] IPTC Core
    [Iptc4xmpExt] IPTC Extension
    [plus] Licences PLUS
    [GPano] Photosphère
    [GCamera] Appareil photo Google
    [hdrgm] Carte de gain HDR
    [mwg-rs] Régions
    [pdf] PDF
    [stEvt] Événements de ressource
    [stRef] Références de ressource
   *[other] Autre
}
unknown-tag = Balise inconnue { $number } ({ $context })
blob-bytes = { $num ->
    [one] ({ $num } octet)
    [many] ({ $num } octets)
   *[other] ({ $num } octets)
}
maker-note-bytes = ({ $num ->
        [one] { $num } octet
       *[other] { $num } octets
    }, note du fabricant, non décodée)
more-values = { $shown }, … ({ $total ->
        [one] { $total } valeur
       *[other] { $total } valeurs
    })

## Liaisons génériques

list-separator = {", "}
one-line-separator = {" · "}

## Exposition

exposure-time-fraction = 1/{ $denominator } s
exposure-time-seconds = { $seconds } s
aperture-value = f/{ $f }
iso-value = ISO { $iso }
focal-length-value = { $mm } mm
focal-length-with-equiv = { $mm } mm (équiv. { $equiv } mm)
focal-length-equiv-only = équiv. { $equiv } mm
exposure-bias-value = { $sign ->
    [plus] +{ $ev } EV
    [minus] −{ $ev } EV
   *[zero] 0 EV
}
exposure-program = { $value ->
    [1] Manuel
    [2] Programme (auto)
    [3] Priorité à l’ouverture
    [4] Priorité à la vitesse
    [5] Créatif (profondeur de champ)
    [6] Action (vitesse rapide)
    [7] Portrait
    [8] Paysage
   *[other] Inconnu
}
metering-mode = { $value ->
    [1] Moyenne
    [2] Moyenne pondérée centrale
    [3] Spot
    [4] Multi-spot
    [5] Matricielle (évaluative)
    [6] Partielle
   *[other] Autre
}
# machine-drafted, needs native review
# Canon maker note CameraSettings FocusMode; 256–258 are the live-view variants of 0–2.
canon-focus-mode = { $value ->
    [0] AF One-Shot
    [1] AF AI Servo
    [2] AF AI Focus
    [3] Mise au point manuelle
    [4] Ponctuel
    [5] Continu
    [6] Mise au point manuelle
    [16] Pan Focus
    [256] AF One-Shot (visée par l’écran)
    [257] AF AI Servo (visée par l’écran)
    [258] AF AI Focus (visée par l’écran)
    [512] Mise au point Movie Snap
    [519] AF Servo vidéo
   *[other] Autre
}
# Canon maker note CameraSettings ImageStabilization; 256–260 repeat 0–4.
canon-stabilisation = { $value ->
    [0] Désactivée
    [1] Activée
    [2] À la prise de vue uniquement
    [3] Filé
    [4] Dynamique
    [256] Désactivée
    [257] Activée
    [258] À la prise de vue uniquement
    [259] Filé
    [260] Dynamique
   *[other] Autre
}
# machine-drafted, needs native review
panasonic-focus-mode = { $value ->
    [1] Auto
    [2] Mise au point manuelle
    [4] Auto (bouton de mise au point)
    [5] Auto (continu)
    [6] AF-S
    [7] AF-C
    [8] AF-F
   *[other] Autre
}
# machine-drafted, needs native review
panasonic-stabilisation = { $value ->
    [2] Activé (optique)
    [3] Désactivé
    [4] Activé (mode 2)
    [5] Activé (optique, filé)
    [6] Activé (boîtier seul)
    [7] Activé (boîtier seul, filé)
    [9] Dual IS
    [10] Dual IS (filé)
    [11] Dual IS 2
    [12] Dual IS 2 (filé)
   *[other] Autre
}
label-comment = Commentaire
fields-tags = Balises { $kind }
group-picture = Réglages de l’image
fields-fujifilm = Notes du fabricant Fujifilm
label-film-simulation = Simulation de film
label-dynamic-range = Plage dynamique
label-highlight-tone = Hautes lumières
label-shadow-tone = Ombres
label-colour-setting = Couleur
label-sharpness = Netteté
label-noise-reduction = Réduction du bruit
label-clarity = Clarté
label-grain-effect = Effet grain
label-colour-chrome = Effet couleur chrome
label-colour-chrome-blue = Couleur chrome bleu
label-shutter-type = Type d’obturateur
label-drive-mode = Mode d’entraînement
fuji-step = { $sign ->
    [plus] +{ $n }
    [minus] −{ $n }
   *[zero] 0
}
fuji-film-mode = { $value ->
    [0] Provia (standard)
    [256] Portrait studio
    [272] Portrait studio (plus saturé)
    [288] Astia (doux)
    [304] Portrait studio (plus net)
    [512] Velvia (vif)
    [768] Portrait studio Ex
    [1024] Velvia
    [1280] Pro Neg. Std
    [1281] Pro Neg. Hi
    [1536] Classic Chrome
    [1792] Eterna
    [2048] Classic Neg.
    [2304] Eterna Bleach Bypass
    [2560] Nostalgic Neg.
    [2816] Reala Ace
   *[other] Autre
}
fuji-monochrome = { $value ->
    [768] Monochrome
    [769] Monochrome, filtre rouge
    [770] Monochrome, filtre jaune
    [771] Monochrome, filtre vert
    [784] Sépia
    [1280] Acros
    [1281] Acros, filtre rouge
    [1282] Acros, filtre jaune
    [1283] Acros, filtre vert
   *[other] Monochrome
}
fuji-white-balance = { $value ->
    [0] Auto
    [1] Auto (priorité au blanc)
    [2] Auto (priorité à l’ambiance)
    [256] Lumière du jour
    [512] Nuageux
    [768] Fluorescent lumière du jour
    [769] Fluorescent blanc jour
    [770] Fluorescent blanc
    [771] Fluorescent blanc chaud
    [772] Fluorescent blanc chaud de salon
    [1024] Incandescent
    [1280] Flash
    [1536] Sous l’eau
    [3840] Personnalisé 1
    [3841] Personnalisé 2
    [3842] Personnalisé 3
    [3843] Personnalisé 4
    [3844] Personnalisé 5
    [4080] Température de couleur
   *[other] Autre
}
fuji-white-balance-kelvin = { $kelvin } K
fuji-dynamic-range = DR{ $percent }
fuji-dynamic-range-auto = Auto (DR{ $percent })
fuji-dynamic-range-setting = { $value ->
    [0] Auto
    [256] Standard (DR100)
    [512] Large 1 (DR230)
    [513] Large 2 (DR400)
   *[other] Autre
}
fuji-effect = { $value ->
    [0] Désactivé
    [32] Faible
    [64] Fort
   *[other] Autre
}
fuji-grain-size = { $value ->
    [16] petit
    [32] grand
   *[other] autre
}
fuji-grain = { $strength }, { $size }
fuji-shutter-type = { $value ->
    [0] Mécanique
    [1] Électronique
    [2] Électronique (poses longues)
    [3] Premier rideau électronique
   *[other] Autre
}
fuji-drive-mode = { $value ->
    [0] Vue par vue
    [1] Rafale, basse vitesse
    [2] Rafale, haute vitesse
   *[other] Autre
}
fuji-focus-mode = { $value ->
    [0] Manuel
    [1] AF-S
    [2] AF-C
    [100] Auto
    [101] Manuel
   *[other] Autre
}
fuji-stabilisation = { $kind } ({ $mode })
fuji-stabilisation-kind = { $value ->
    [0] Aucune
    [1] Optique
    [2] Capteur mobile
    [3] Objectif
    [258] Capteur mobile et numérique
    [512] Numérique
   *[other] Autre
}
fuji-stabilisation-mode = { $value ->
    [0] Désactivé
    [1] activée, en continu
    [2] activée, à la prise de vue
   *[other] Autre
}
light-source = { $value ->
    [1] Lumière du jour
    [2] Fluorescent
    [3] Tungstène
    [4] Flash
    [9] Beau temps
    [10] Nuageux
    [11] Ombre
    [12] Fluorescent lumière du jour
    [13] Fluorescent blanc jour
    [14] Fluorescent blanc froid
    [15] Fluorescent blanc
    [16] Fluorescent blanc chaud
    [17] Lumière standard A
    [18] Lumière standard B
    [19] Lumière standard C
    [20] D55
    [21] D65
    [22] D75
    [23] D50
    [24] Tungstène de studio
   *[other] Autre
}
white-balance-auto = Automatique
white-balance-manual = Manuelle
white-balance-manual-with-light = Manuelle ({ $light })
flash-none = Pas de fonction flash
flash-fired = Flash déclenché
flash-not-fired = Flash non déclenché
flash-forced-on = déclenchement forcé
flash-forced-off = désactivation forcée
flash-auto = mode automatique
flash-return-not-detected = lumière de retour non détectée
flash-return-detected = lumière de retour détectée
flash-red-eye = réduction des yeux rouges

## Objectif

lens-focal-single = { $mm } mm
lens-focal-range = { $min }–{ $max } mm
lens-aperture-single = f/{ $f }
lens-aperture-range = f/{ $min }–{ $max }
lens-range = { $focal } { $aperture }

## Date et heure

date-with-weekday = { $weekday } { $date }
date-time = { $date } à { $time }
date-time-zone = { $datetime } ({ $zone })
zone-utc = UTC
zone-offset = UTC{ $offset }
gps-date-time = { $date } à { $time } UTC
detail-zone-from-gps = Fuseau horaire déduit de l’horloge GPS

## Lieu

coordinates = { $lat }° { $latRef ->
    [south] S
   *[north] N
}, { $lon }° { $lonRef ->
    [west] O
   *[east] E
}
altitude = { $ref ->
    [above] { $m } m au-dessus du niveau de la mer
    [below] { $m } m au-dessous du niveau de la mer
   *[sea] Au niveau de la mer
}
compass-point = { $point ->
    [n] nord
    [nne] nord-nord-est
    [ne] nord-est
    [ene] est-nord-est
    [e] est
    [ese] est-sud-est
    [se] sud-est
    [sse] sud-sud-est
    [s] sud
    [ssw] sud-sud-ouest
    [sw] sud-ouest
    [wsw] ouest-sud-ouest
    [w] ouest
    [wnw] ouest-nord-ouest
    [nw] nord-ouest
   *[nnw] nord-nord-ouest
}
facing = Orienté vers le { $point } ({ $degrees }°)
detail-true-north = Par rapport au nord géographique
detail-magnetic-north = Par rapport au nord magnétique
accuracy = ±{ $m } m
distance-km = { $km } km
place = { $city }, { $country }
place-city-only = { $city }
place-near = près de { $place }
place-detail = { $areas } · à { $distance } de { $city }
place-detail-distance = à { $distance } de { $city }

## Image

dimensions = { $width } × { $height }
dimensions-megapixels = { $width } × { $height } ({ $mp } Mpx)
encoding-bit-depth = { $bits } bits
encoding-progressive = { $format }, progressif
encoding-baseline = { $format }, ligne de base
encoding-hierarchical = { $format }, hiérarchique
# machine-drafted, needs native review
encoding-alpha = { $format } { $alpha ->
    [premultiplied] avec alpha prémultiplié
    [transparency] avec transparence
   *[alpha] avec alpha
}
# [Lab], [Palette] and the variants after them: machine-drafted, needs native review
colour-model = { $model ->
    [YCbCr] YCbCr
    [RGB] RVB
    [Greyscale] Niveaux de gris
    [CMYK] CMJN
    [YCCK] YCCK
    [Lab] L*a*b*
    [Palette] Couleurs indexées
    [Multichannel] Multicouche
    [Duotone] Bichromie
    [YCgCo] YCgCo
    [XYB] XYB
    [YC] Luminance/chrominance (YC)
    [Bayer] RAW Bayer
    [X-Trans] RAW X-Trans
    [CFA] RAW à matrice de filtres colorés
    [LinearRaw] RAW linéaire
    [RGBE] RGBE (exposant partagé)
    [XYZE] XYZE (exposant partagé)
    [R] Un canal (R)
    [RG] Deux canaux (RG)
   *[other] { $model }
}
adobe-rgb-from-exif = Adobe RGB (d’après la balise EXIF ; aucun profil incorporé)
orientation = { $value ->
    [1] Normale (aucune rotation nécessaire)
    [2] Miroir horizontal (appliqué)
    [3] Rotation de 180° (appliquée)
    [4] Miroir vertical (appliqué)
    [5] Miroir horizontal et rotation de 90° dans le sens antihoraire (appliqués)
    [6] Rotation de 90° dans le sens horaire (appliquée)
    [7] Miroir horizontal et rotation de 90° dans le sens horaire (appliqués)
    [8] Rotation de 90° dans le sens antihoraire (appliquée)
   *[other] Inconnue ({ $value })
}
orientation-short = { $value ->
    [1] Normale
    [2] Miroir horizontal
    [3] Rotation de 180°
    [4] Miroir vertical
    [5] Miroir horizontal et rotation de 90° dans le sens antihoraire
    [6] Rotation de 90° dans le sens horaire
    [7] Miroir horizontal et rotation de 90° dans le sens horaire
    [8] Rotation de 90° dans le sens antihoraire
   *[other] valeur { $value }
}

## Fichier

file-size-bytes = { $num ->
    [one] { $num } octet
    [many] { $num } octets
   *[other] { $num } octets
}
# Unités décimales (1 ko = 1000 octets).
file-size-unit = { $num } { $unit ->
    [kb] ko
    [mb] Mo
    [gb] Go
    [tb] To
   *[pb] Po
}

## Droits

rating-rejected = Rejetée
rating-stars = { $stars } ({ $n } sur 5)

## Identifiants de contenu

c2pa-present = Présents (non vérifiés)
c2pa-detail = Ce fichier contient un manifeste C2PA ; sa signature n’a pas été vérifiée.

## Valeurs de l’en-tête ICC

icc-class = { $class ->
    [scnr] Périphérique d’entrée
    [mntr] Périphérique d’affichage
    [prtr] Périphérique de sortie
    [link] Liaison de périphériques
    [spac] Espace colorimétrique
    [abst] Abstrait
    [nmcl] Couleur nommée
   *[other] Inconnue
}
icc-intent = { $intent ->
    [0] Perceptif
    [1] Colorimétrie relative
    [2] Saturation
    [3] Colorimétrie absolue
   *[other] Inconnu
}
icc-platform = { $platform ->
    [APPL] Apple
    [MSFT] Microsoft
    [SGI] Silicon Graphics
    [SUNW] Sun Microsystems
    [none] Non spécifiée
   *[other] Inconnue
}
icc-coded = { $name } ({ $code })

# machine-drafted, needs native review
label-modality = Modalité
label-body-part = Région anatomique
label-study = Étude
label-series = Série
label-slice-thickness = Épaisseur de coupe
label-pixel-spacing = Espacement des pixels
label-imager-pixel-spacing = Espacement des pixels au détecteur
label-frames = Trames
label-transfer-syntax = Syntaxe de transfert
label-document = Document
label-document-type = Type de document
medical-spacing = { $row } × { $column } mm
medical-mm = { $mm } mm
medical-lossy = { $name } (compression avec pertes)
detail-pixel-spacing = Distance entre les centres des pixels dans le patient (lignes × colonnes).
# machine-drafted, needs native review
# Pixel spacing of an industrial inspection image (DICONDE, CT of an object).
detail-pixel-spacing-object = Distance entre les centres des pixels dans l’objet (lignes × colonnes).
detail-imager-pixel-spacing = Mesuré au détecteur, sans correction de l’agrandissement : les distances dans le patient sont en général plus petites.

## Contrôle industriel (DICONDE)

# machine-drafted, needs native review
label-nde-component-shape = Forme de la pièce
label-nde-component-manufacturer = Fabricant de la pièce
label-nde-material = Matériau
label-nde-thickness = Épaisseur du matériau
label-nde-pipe-diameter = Diamètre du tube
label-nde-source = Source
label-nde-environment = Environnement
label-nde-iqi = Indicateur de qualité d’image
label-nde-indications = Indications
label-nde-standard = Norme

## Volumes de recherche (NIfTI, Analyze, NRRD, MetaImage)

# machine-drafted, needs native review
label-voxel-size = Taille de voxel
label-data-type = Type de données
label-time-points = Points temporels
label-orientation-source = Orientation d’après
label-intent = Valeurs
label-compression = Compression
volume-voxel-size = { $x } × { $y } × { $z } mm
volume-orient-sform = NIfTI sform
volume-orient-qform = NIfTI qform
volume-orient-analyze = En-tête Analyze (peu fiable)
volume-orient-space = Directions spatiales NRRD
volume-orient-transform = Transformation MetaImage
volume-orient-spacings = Espacement des voxels seul
volume-orient-none = Non indiquée
volume-orient-vox2ras = FreeSurfer vox2ras
volume-orient-mgh-default = Valeur par défaut FreeSurfer (peu fiable)
volume-orient-minc = Cosinus directeurs MINC
volume-orient-object = Coordonnées de l’objet (pas d’orientation patient)
volume-content-projections = Projections (acquisition brute)
volume-content-reconstruction = Volume reconstruit
volume-coords-rectilinear = Rectiligne (espacement irrégulier, affiché uniforme)
# Scan parameters and tags a FreeSurfer MGH file keeps after its voxels.
label-volume-tr = Temps de répétition
label-volume-te = Temps d’écho
label-volume-ti = Temps d’inversion
label-volume-flip = Angle de bascule
label-volume-fov = Champ de vue
label-volume-field = Intensité du champ
label-volume-pedir = Codage de phase
label-volume-cmdline = Ligne de commande
label-volume-xform = Transformation de Talairach
label-volume-content = Contenu
label-volume-projections = Projections
label-volume-angles = Plage de rotation
label-volume-angle-step = Pas de rotation
label-volume-kv = Tension du tube
label-volume-current = Courant du tube
label-volume-power = Puissance de la source
label-volume-exposure = Exposition
label-volume-sod = Source–objet
label-volume-odd = Objet–détecteur
label-volume-sdd = Source–détecteur
label-volume-magnification = Grossissement optique
label-volume-binning = Binning
label-volume-objective = Objectif
label-volume-filter = Filtre de la source
label-volume-camera = Détecteur
label-volume-instrument = Instrument
label-volume-software = Logiciel
label-volume-date = Acquis le
label-volume-ring = Correction des artefacts en anneau
label-volume-smoothing = Lissage
label-volume-coords = Grille
# machine-drafted, needs native review
# An eye scan (OCT): which eye.
label-volume-eye = Œil
volume-eye-right = Droit (OD)
volume-eye-left = Gauche (OS)
detail-volume-unreliable = Le fichier n’indique pas où pointent ses axes dans le patient : la gauche et la droite peuvent être inversées, aucun repère d’orientation n’est donc affiché.
# machine-drafted, needs native review
# An industrial CT volume: its axes are the scanner's, there is no patient.
detail-volume-object = Les axes sont ceux du scanner, pas ceux d’un patient : aucun repère d’orientation n’est donc affiché.

## Images scientifiques (FITS, XISF)

label-object = Objet
label-telescope = Télescope
label-instrument = Instrument
label-filter = Filtre
label-observed = Observé
label-exposure-time = Temps de pose
label-sample = Échantillons
label-unit = Unité
label-hdu = Extension
label-centre = Centre
label-centre-deg = Centre (degrés)
label-frame = Repère de coordonnées
# machine-drafted, needs native review
science-frame-galactic = Galactique
science-frame-ecliptic = Écliptique
label-projection = Projection
label-pixel-scale = Échelle du pixel
science-seconds = { $seconds } s
science-arcsec-px = { $arcsec }″ par pixel
label-distortion = Distorsion
label-wcs = Coordonnées célestes
science-wcs-unsupported = Non pris en charge : { $reason }
# machine-drafted, needs native review
science-wcs-approximate = Approximatives (pointage seul, sans distorsion)
label-observer = Observateur
label-bayer = Matrice de Bayer
label-capture-start = Début de capture (UTC)
label-capture-end = Fin de capture (UTC)
label-frame-rate = Fréquence d’images
science-fps = { $fps } images/s
science-frames-cut = { $frames } sur { $declared }
# The HDF5 dataset or NetCDF variable shown.
label-dataset = Jeu de données
label-pixel-size = Taille de pixel
# "0.25 nm per pixel"; $size may be "0.25 × 0.5".
science-pixel-size = { $size } { $unit } par pixel
# Microscopy (DigitalMicrograph, MRC, Gwyddion, Igor).
label-microscope = Microscope
label-voltage = Tension d’accélération
label-magnification = Grossissement
label-imaging-mode = Mode
label-detector = Détecteur
label-acquired = Acquis
label-field-of-view = Champ de vue
science-kilovolts = { $kv } kV
science-magnification = { $value }×
label-vendor = Fabricant
label-scanner = Scanner
label-objective = Objectif
label-pyramid-levels = Niveaux
# machine-drafted, needs native review
label-emissivity = Émissivité
label-object-distance = Distance de l’objet
label-reflected-temperature = Température réfléchie
label-atmospheric-temperature = Température atmosphérique
label-humidity = Humidité relative
label-raw-range = Plage brute
label-temperature-range = Plage de températures
thermal-temperature = { $value } { $unit }
thermal-metres = { $value } m
thermal-percent = { PERCENT($value) }
thermal-range = { $low } – { $high }
slide-levels = { $count } : { $sizes }
# X-ray diffraction: the experiment geometry from a detector frame's header.
label-wavelength = Longueur d’onde
# Ångström, e.g. "0.97625 Å".
xtal-angstrom = { $value } Å
label-photon-energy = Énergie des photons
xtal-kev = { $value } keV
# Sample to detector distance.
label-detector-distance = Distance du détecteur
xtal-mm = { $value } mm
# Where the direct beam meets the detector, in pixels from the image's top-left corner.
label-beam-centre = Centre du faisceau
xtal-beam-centre = x { $x }, y { $y } px
# The detector arm's swing angle (2θ).
label-detector-swing = 2θ du détecteur
xtal-degrees = { $value }°
# Goniometer angles (Greek letters are the field's usual names).
label-phi = Phi (φ)
label-omega = Oméga (ω)
label-chi = Khi (χ)
label-kappa = Kappa (κ)
# The sample's rotation during the exposure: start angle plus width, e.g. "12.5° + 0.1°".
label-oscillation = Oscillation
xtal-oscillation = { $start }° + { $width }°
# $axis is the axis name from the file, e.g. "phi" or "omega".
xtal-oscillation-axis = { $start }° + { $width }° autour de { $axis }
# The count at which a pixel saturates.
label-overload = Seuil de saturation
# The finest lattice spacing (d) recorded at the detector's edge and in its corners.
label-resolution = Résolution
xtal-resolution = { $edge } Å au bord, { $corner } Å dans les coins
label-geometry = Géométrie
xtal-curved = Plaque image courbe : ni q, ni d, ni anneaux de résolution
xtal-incomplete = En-tête incomplet : ni q, ni d, ni anneaux de résolution
# All-fields group title: "FITS header".
fields-science-header = En-tête { $format }
# machine-drafted, needs native review
fields-asdf-tree = Arborescence ASDF
label-asdf-model = Modèle de données
label-asdf-version = Version du fichier
label-asdf-standard = Norme ASDF
label-asdf-library = Écrit par
# machine-drafted, needs native review
# All-fields group title for a CERN ROOT file's list of keys (objects).
fields-root-keys = Clés ROOT
# CERN ROOT files: the ROOT version that wrote the file (e.g. "6.28/04"), its compression,
# the number of keys (stored objects), and the object shown: class (e.g. "TH2F"), its path in
# the file, title, number of entries and sum of weights.
label-root-version = Version de ROOT
label-root-compression = Compression
label-root-keys = Clés
label-root-class = Classe
label-root-name = Nom
label-root-title = Titre
label-root-entries = Entrées
label-root-sum-weights = Somme des poids
# A graph's number of points.
label-root-points = Points
# $axis is "x", "y" or "z".
label-root-axis = Axe { $axis }
# $quantity is the axis title with its unit, e.g. "Energy [GeV]"; $min and $max the axis
# limits.
root-axis-fixed = { $quantity } : { $bins ->
        [one] { $bins } bin
        [many] { $bins } bins
       *[other] { $bins } bins
    }, de { $min } à { $max }
root-axis-variable = { $quantity } : { $bins ->
        [one] { $bins } bin variable
        [many] { $bins } bins variables
       *[other] { $bins } bins variables
    }, de { $min } à { $max }
label-root-mean-rms = Moyenne et RMS { $axis }
root-mean-rms = { $mean } ± { $rms }
# Trees (event data) are listed only, with their entry counts.
label-root-trees = Arbres
root-tree = { $name } ({ $entries ->
        [one] { $entries } entrée
        [many] { $entries } entrées
       *[other] { $entries } entrées
    })
# machine-drafted, needs native review
# CASA images: what the image is (from its directory name), its restoring beam (major and
# minor axis, position angle), the beams of its planes when they differ, the rest frequency,
# spectral reference frame (e.g. "LSRK") and velocity definition of the frequency labels,
# the default mask (e.g. "mask0") and the casacore storage manager with its byte order.
label-casa-type = Type d’image
casa-type-image = Image restaurée
casa-type-residual = Résidu
casa-type-model = Modèle
casa-type-pb = Lobe primaire
casa-type-mask = Masque de nettoyage
casa-type-psf = Fonction d’étalement du point
casa-type-sumwt = Somme des poids
casa-type-weight = Poids
label-casa-beam = Lobe de restauration
casa-beam = { $major }″ × { $minor }″, AP { $pa }°
label-casa-plane-beams = Lobes par plan
casa-plane-beams = { $count ->
        [one] { $count } lobe
        [many] { $count } de lobes
       *[other] { $count } lobes
    }, grand axe de { $min }″ à { $max }″
label-casa-rest-frequency = Fréquence au repos
casa-ghz = { $ghz } GHz
label-casa-spectral-frame = Référentiel spectral
label-casa-velocity = Définition de la vitesse
casa-velocity-radio = Radio
casa-velocity-optical = Optique
casa-velocity-relativistic = Relativiste
label-casa-mask = Masque par défaut
label-casa-storage = Stockage
casa-storage-big = { $manager }, gros-boutiste
casa-storage-little = { $manager }, petit-boutiste
# machine-drafted, needs native review
# Unstructured meshes: the mesh's name in the file, the convention describing it (UGRID,
# ICON, MPAS), its counts, face shapes, coordinates and where the values sit.
label-ugrid-name = Maillage
label-ugrid-convention = Convention
label-ugrid-nodes = Nœuds
label-ugrid-faces = Faces
label-ugrid-edges = Arêtes
label-ugrid-shapes = Formes des faces
label-ugrid-coordinates = Coordonnées
label-ugrid-values = Valeurs sur
label-ugrid-split = Découpé pour l’affichage
# $n is the formatted count, $count the number (for the plural).
ugrid-triangles = { $count ->
    [one] { $count } triangle
    [many] { $count } de triangles
   *[other] { $count } triangles
}
ugrid-quads = { $count ->
    [one] { $count } quadrilatère
    [many] { $count } de quadrilatères
   *[other] { $count } quadrilatères
}
# Faces of five or more nodes; $max is the most nodes of one face.
ugrid-polygons = { $count ->
    [one] { $count } polygone de { $max } nœuds
    [many] { $count } de polygones d’au plus { $max } nœuds
   *[other] { $count } polygones d’au plus { $max } nœuds
}
ugrid-lonlat = Longitude et latitude
ugrid-projected = x et y projetés
ugrid-projected-unit = x et y projetés ({ $unit })
ugrid-on-faces = Faces (rendu uniforme)
ugrid-on-nodes = Nœuds (interpolés sur les faces)
# A mesh file without data variables: the image shows how many nodes each face has.
ugrid-grid-only = Aucune : l’image montre le nombre de nœuds par face
# Faces cut at the antimeridian or closed to a pole so they draw on a flat map.
ugrid-split = { $count ->
    [one] { $count } face à l’antiméridien ou à un pôle
    [many] { $count } de faces à l’antiméridien ou à un pôle
   *[other] { $count } faces à l’antiméridien ou à un pôle
}
# machine-drafted, needs native review
# Slide annotations (NDP.view, Aperio ImageScope, ASAP sidecar files).
label-annotation-file = Fichier
# "slide.xml (Aperio ImageScope)"
annotation-source = { $file } ({ $format })
label-annotation-shapes = Formes
label-annotation-groups = Groupes
# Shapes or points over the reader's limits, left out.
label-annotation-skipped = Omises
annotation-polygon = Polygone
annotation-polyline = Ligne
annotation-rectangle = Rectangle
annotation-ellipse = Ellipse
annotation-point = Point
annotation-points = Points
annotation-ruler = Règle
annotation-arrow = Flèche
# The list of annotations is cut short: "and 1,200 more".
annotation-more = et { $count } de plus
# All-fields group title: "Annotations in slide.xml".
fields-annotations-header = Annotations de { $file }
label-drawing-version = Version
label-drawing-units = Unités
label-drawing-extents = Étendue
label-drawing-sheet = Feuille
label-drawing-layouts = Présentations
label-drawing-layers = Calques
label-drawing-entities = Entités
label-drawing-codepage = Page de codes
label-drawing-missing-fonts = Polices substituées
# machine-drafted, needs native review
# A font file the drawing names that was not found: the styles naming it, and the font drawing its text instead.
drawing-missing-font = { $file } (styles { $styles }), remplacée par { $substitute }
# A Big Font (double-byte text) that was not found; its characters come from other fonts.
drawing-missing-bigfont = { $file } (styles { $styles }), remplacée par des polices de secours
# Survey data (LandXML).
label-survey-crs = Système de coordonnées
survey-crs-none = Non indiqué
label-survey-project = Nom du projet
label-survey-application = Écrit par
label-survey-surface = Modèle de terrain
# A TIN surface: its name, counts, plan and sloped areas, the volume above its lowest point, its elevations and the contour interval drawn; unit is "m" or "ft" (areas in square units, volume in cubic units).
survey-surface = { $name } : { $points ->
        [one] { $points } point
       *[other] { $points } points
    }, { $triangles ->
        [one] { $triangles } triangle
       *[other] { $triangles } triangles
    } ; surface en plan { $area } { $unit }², surface réelle { $area3d } { $unit }² ; { $volume } { $unit }³ au-dessus du point le plus bas ; altitudes { $zmin }–{ $zmax } { $unit } ; courbes de niveau tous les { $interval } { $unit }
label-survey-alignment = Axe en plan
# An alignment: its name, length and first and last stations ("1+234.50").
survey-alignment = { $name } : { $length } { $unit }, PK { $start } à { $end }
# SHX shape fonts found and used for the drawing's text.
label-drawing-shx-fonts = Polices SHX
label-drawing-warnings = Problèmes
label-plot-paper = Papier
# "2 of 5": the page shown and how many the plot has.
plot-page-of = { $page } sur { $pages }
label-plot-page = Page
label-plot-pens = Plumes utilisées
# The printer language a plot came wrapped in, e.g. "PCL 5".
label-plot-job = Tâche d’impression
# What a plot draws: strokes, fills and labels.
label-plot-items = Éléments tracés
# Width × height of what is drawn, in drawing units: "1,250 × 800 mm".
drawing-extents = { $width } × { $height } { $unit }
drawing-extents-unitless = { $width } × { $height }
drawing-model-space = Modèle
# "24, 3 off or frozen"
drawing-layers-off = { $num }, dont { $off } désactivés ou gelés
# DXF $INSUNITS.
drawing-units = { $code ->
    [1] Pouces
    [2] Pieds
    [3] Miles
    [4] Millimètres
    [5] Centimètres
    [6] Mètres
    [7] Kilomètres
    [8] Micropouces
    [9] Millièmes de pouce
    [10] Yards
    [11] Ångströms
    [12] Nanomètres
    [13] Microns
    [14] Décimètres
    [15] Décamètres
    [16] Hectomètres
    [17] Gigamètres
    [18] Unités astronomiques
    [19] Années-lumière
    [20] Parsecs
    [21] Pieds d’arpentage US
   *[other] Sans unité
}
# All-fields group title: "DXF entities".
fields-drawing-entities = Entités { $format }

## Layouts de circuits intégrés (GDSII)

# machine-drafted, needs native review
label-layout-library = Bibliothèque
label-layout-units = Unités
# The database unit (the grid coordinates are stored in) and the user unit, with SI symbols:
# "1 nm database unit, 1 µm user unit".
layout-units = { $db } unité de base de données, { $user } unité utilisateur
# The top cell shown (each top cell is a page).
label-layout-cell = Cellule
label-layout-cells = Cellules
# Cells no other cell places.
label-layout-top-cells = Cellules racines
# "TOP, TEST, PAD and 12 more"
layout-top-cells-more = { $names } et { $more } autres
# Layer/datatype pairs.
label-layout-layers = Couches
# Polygons, paths and boxes, as stored (each cell once).
label-layout-shapes = Formes
label-layout-texts = Étiquettes
# Cell references and the placements they make: "1,204 (88,320 placements)".
label-layout-references = Références
layout-references = { $refs } ({ $placements } { $placements ->
    [one] placement
    [many] de placements
   *[other] placements
})
# Shapes with every cell placement expanded.
label-layout-flat-shapes = Formes après mise à plat
label-layout-bounds = Emprise
# Corners of what the cell draws, micrometres: "-12.5, 0 to 2,900, 3,500 µm".
layout-bounds = { $x0 }, { $y0 } à { $x1 }, { $y1 } µm

## Circuits imprimés et schémas (Gerber, Excellon, KiCad)

# machine-drafted, needs native review
group-board = Circuit imprimé
group-schematic = Schéma
# The board view shown (its layers, its top or bottom), or the schematic sheet.
label-pcb-view = Vue
label-pcb-project = Projet
label-pcb-revision = Révision
label-pcb-date = Date
# The CAD program that wrote the file.
label-pcb-generator = Créé avec
# Width × height of the board outline: "100 × 80 mm".
label-pcb-size = Taille de la carte
label-pcb-thickness = Épaisseur
label-pcb-copper-layers = Couches de cuivre
label-pcb-sheets = Feuilles
# The board's layers or files, each with its role: "board-F_Cu.gbr (Top copper)".
label-pcb-layers = Couches
pcb-layer-role = { $name } ({ $role })
label-pcb-components = Empreintes
label-pcb-symbols = Symboles
label-pcb-nets = Équipotentielles
label-pcb-tracks = Pistes
label-pcb-wires = Fils
label-pcb-vias = Vias
# Drilled holes and how many drill sizes: "1,204 in 9 sizes".
label-pcb-holes = Trous
pcb-holes = { $holes } en { $sizes } { $sizes ->
    [one] diamètre
    [many] de diamètres
   *[other] diamètres
}
label-pcb-apertures = Ouvertures
# What a board layer or file is, shown after its name.
pcb-role-inner-copper = Cuivre interne { $n }
pcb-role-top-copper = Cuivre dessus
pcb-role-bottom-copper = Cuivre dessous
pcb-role-top-solder-mask = Vernis épargne dessus
pcb-role-bottom-solder-mask = Vernis épargne dessous
pcb-role-top-silkscreen = Sérigraphie dessus
pcb-role-bottom-silkscreen = Sérigraphie dessous
pcb-role-top-paste = Pâte dessus
pcb-role-bottom-paste = Pâte dessous
pcb-role-top-solder-paste = Pâte à braser dessus
pcb-role-bottom-solder-paste = Pâte à braser dessous
pcb-role-top-adhesive = Colle dessus
pcb-role-bottom-adhesive = Colle dessous
pcb-role-top-courtyard = Zone d’encombrement dessus
pcb-role-bottom-courtyard = Zone d’encombrement dessous
pcb-role-top-fabrication = Fabrication dessus
pcb-role-bottom-fabrication = Fabrication dessous
pcb-role-board-outline = Contour de la carte
pcb-role-plated-drill = Perçage métallisé
pcb-role-non-plated-drill = Perçage non métallisé
pcb-role-drill = Perçage
pcb-role-drill-holes = Trous de perçage
pcb-role-mechanical = Mécanique
pcb-role-margin = Marge
pcb-role-drawings = Dessins
pcb-role-comments = Commentaires
pcb-role-engineering-change-1 = Modification 1
pcb-role-engineering-change-2 = Modification 2
pcb-role-vias = Vias
pcb-role-other = Autre
pcb-role-unknown = Inconnu
label-pcb-macros = Macros d’ouverture

## Spectres (JCAMP-DX, SPC, mzML, mzXML, Bruker NMR)

# machine-drafted, needs native review
group-spectrum = Spectre
label-spectrum-format = Format
label-spectrum-technique = Technique
label-spectrum-title = Titre
# The page shown: "3 of 1,204: Scan 1532".
label-spectrum-page = Page
spectrum-page-of = { $page } sur { $pages } : { $name }
# An axis with its unit: "Wavenumber (cm⁻¹)".
spectrum-axis = { $title } ({ $unit })
label-spectrum-x = Axe X
label-spectrum-y = Axe Y
label-spectrum-nucleus = Noyau
label-spectrum-frequency = Fréquence du spectromètre
label-spectrum-solvent = Solvant
label-spectrum-pulse-program = Séquence d’impulsions
label-spectrum-spectra = Spectres
label-spectrum-points = Points
label-spectrum-instrument = Instrument
# Who or what lab measured it.
label-spectrum-origin = Origine
label-spectrum-date = Mesuré le
# How Specular made the page from the raw data (an FID transformed, phased automatically).
label-spectrum-processing = Traitement
spectrum-technique-ir = Infrarouge
spectrum-technique-raman = Raman
spectrum-technique-uv-vis = UV-Vis
spectrum-technique-fluorescence = Fluorescence
spectrum-technique-nmr = RMN
spectrum-technique-ms = Spectre de masse
spectrum-technique-chromatogram = Chromatogramme
spectrum-technique-other = Spectre

## Modèles 3D (STL, OBJ, PLY, glTF, 3MF)

# machine-drafted, needs native review
group-mesh = Modèle 3D
label-triangles = Triangles
label-vertices = Sommets
label-materials = Matériaux
label-textures = Textures
label-up-axis = Axe vertical
# Textures the model uses, some of which could not be read: "3 (1 missing)".
mesh-textures-missing = { $count } ({ $missing } manquantes)

## Structures moléculaires (PDB, mmCIF, CIF, SDF, XYZ)

# machine-drafted, needs native review
group-structure = Structure
# The entry's identifier: a PDB ID such as 1CRN, or the CIF data block's name.
label-structure-id = Entrée
label-structure-classification = Classification
# How the structure was determined: X-ray diffraction, electron microscopy, solution NMR…
label-structure-method = Méthode
# The crystallographic or cryo-EM resolution, in ångströms.
label-structure-resolution = Résolution
structure-resolution = { $value } Å
label-structure-deposited = Déposée
label-structure-formula = Formule
label-structure-chains = Chaînes
# Residues (amino acids, nucleotides) in the polymer chains.
label-structure-residues = Résidus
label-structure-atoms = Atomes
# Atoms after the unit cell was filled by symmetry, and those the file lists: "96 (12 in the asymmetric unit)".
structure-atoms-cell = { $count } ({ $asymmetric } dans l’unité asymétrique)
label-structure-bonds = Liaisons
# Models in the file (an NMR ensemble), of which the first is shown.
label-structure-models = Modèles
# Molecules (SDF records) or data blocks in the file, of which the first is shown.
label-structure-records = Enregistrements
structure-first-of = { $count } (le premier est affiché)
# Small molecules bound to the polymer, by their three-letter codes.
label-structure-ligands = Ligands
# A list cut short: "HEM, NAG, SO4 and 9 more".
structure-ligands-more = { $list } et { $more } autres
label-structure-cell = Maille élémentaire
# Unit cell edge lengths (Å) and angles (degrees).
structure-cell = a = { $a } Å, b = { $b } Å, c = { $c } Å, α = { $alpha }°, β = { $beta }°, γ = { $gamma }°
label-structure-space-group = Groupe d’espace
label-structure-secondary = Structure secondaire
structure-ss-file = Issue du fichier
structure-ss-computed = Calculée à partir de la chaîne principale

## HDF4 et HDF-EOS2

# machine-drafted, needs native review
group-hdf4 = HDF4
# The HDF library version that wrote the file, e.g. "NCSA HDF Version 4.2 Release 15".
label-hdf4-library = Bibliothèque
# Counts of the file's scientific datasets, raster images, groups (Vgroups) and tables (Vdatas).
label-hdf4-datasets = Jeux de données
label-hdf4-images = Images raster
label-hdf4-vgroups = Groupes
label-hdf4-vdatas = Tables
# HDF-EOS: NASA's Earth Observing System conventions on top of HDF4 (MODIS, ASTER, AIRS…).
group-hdf-eos = HDF-EOS
label-eos-version = Version
# The grids, swaths and point structures the file defines.
label-eos-structures = Structures
eos-grids = { $count ->
    [one] { $count } grille ({ $names })
    [many] { $count } grilles ({ $names })
   *[other] { $count } grilles ({ $names })
}
eos-swaths = { $count ->
    [one] { $count } fauchée ({ $names })
    [many] { $count } fauchées ({ $names })
   *[other] { $count } fauchées ({ $names })
}
eos-points = { $count ->
    [one] { $count } structure de points
    [many] { $count } structures de points
   *[other] { $count } structures de points
}
# The grid or swath the dataset shown belongs to.
label-eos-structure = Structure
# The grid's map projection as its GCTP name, e.g. GCTP_SNSOID (sinusoidal).
label-eos-projection = Projection
label-eos-grid-size = Taille de la grille
# Columns × rows.
eos-grid-size = { $width } × { $height }
# The grid's upper-left and lower-right corners.
label-eos-corners = Coins
# The product's short name from the inventory metadata, e.g. MOD09A1.
label-eos-product = Produit
# When the data was acquired.
label-eos-time = Date
eos-time = du { $begin } au { $end }

## Images planétaires (VICAR, PDS3, PDS4, données de missions de la NASA et de l’ESA)

# machine-drafted, needs native review
group-planetary = Image planétaire
# The body observed, e.g. MARS, Ganymede.
label-planetary-target = Cible
label-planetary-mission = Mission
# The spacecraft, rover or lander carrying the instrument.
label-planetary-spacecraft = Engin spatial
label-planetary-start = Heure de début
label-planetary-stop = Heure de fin
# The product's identifier in the archive.
label-planetary-product = ID du produit
# The map projection of a map-projected image, as the label names it.
label-planetary-projection = Projection cartographique
label-planetary-resolution = Résolution
planetary-metres-per-pixel = { $metres } m par pixel
planetary-pixels-per-degree = { $pixels } pixels par degré
# Latitude and longitude of the map projection's centre.
label-planetary-centre = Centre de projection
planetary-lat-lon = { $lat }°, { $lon }°
# The latitudes and longitudes the map covers.
label-planetary-coverage = Couverture
planetary-coverage = latitude { $south }° à { $north }°, longitude { $west }° à { $east }°
# The body's equatorial radius the map uses.
label-planetary-radius = Rayon du corps
planetary-km = { $km } km

## OMERO (images ouvertes depuis un serveur OMERO ou l’Image Data Resource)

# machine-drafted, needs native review
group-omero = OMERO
label-omero-server = Serveur
# $server the server's address; $user the user name signed in with.
omero-server-user = { $server } (en tant que { $user })
omero-server-public = { $server } (public)
label-omero-image = Image
# $name the image's name; $id its number on the server.
omero-image = { $name } (ID { $id })
# The project and dataset the image is in, as "Project › Dataset".
label-omero-project = Projet
# The screen, plate and well, as "Screen › Plate › B4".
label-omero-screen = Criblage
label-omero-owner = Propriétaire
# $owner the image's owner; $group the OMERO group it belongs to.
omero-owner-group = { $owner } ({ $group })
label-omero-acquired = Acquisition
label-omero-dimensions = Dimensions
omero-size = { $width } × { $height }
omero-z = { $count ->
    [one] { $count } plan Z
   *[other] { $count } plans Z
}
omero-c = { $count ->
    [one] { $count } canal
   *[other] { $count } canaux
}
omero-t = { $count ->
    [one] { $count } point temporel
   *[other] { $count } points temporels
}
# The physical size of a pixel along X, Y (and Z).
label-omero-physical = Taille de pixel
label-omero-channels = Canaux
# A channel not shown in the server's rendering.
omero-channel-hidden = { $channel } (masqué)
# How the pixels are fetched from the server: rendered tiles (JPEG) or the whole plane (PNG).
label-omero-fetched = Récupéré en
omero-fetched-tiles = tuiles { $encoding } de { $width } × { $height }, { $levels ->
    [one] { $levels } niveau
   *[other] { $levels } niveaux
}
omero-fetched-whole = Plans entiers en { $encoding }

## Images à transformation de réflectance (PTM, RTI, Relight)

# machine-drafted, needs native review
group-rti = Transformation de réflectance
# The reflectance model the coefficients belong to: PTM (polynomial), HSH (hemispherical harmonics), RBF…
label-rti-basis = Base
label-rti-order = Ordre
# Coefficients per colour channel.
label-rti-terms = Coefficients
# How colour is stored: LRGB (one colour, varying luminance), RGB, YCC…
label-rti-colour = Disposition des couleurs
# Coefficient images stored in the file.
label-rti-planes = Plans
# The byte-to-coefficient mapping the file states, per coefficient.
label-rti-scale = Échelle
label-rti-bias = Décalage

## Nuages de points

# machine-drafted, needs native review
group-points = Nuage de points
label-points = Points
# Points read of those in the file, when a very large cloud was evenly thinned: "32,000,000 of 120,000,000".
points-subsampled = { $loaded } sur { $total }
# The LAS point data record format number (0–10).
label-point-format = Format de point
# The coordinate reference system.
label-crs = Système de coordonnées
# What each point carries besides its position.
label-attributes = Attributs
points-channel-rgb = RVB
points-channel-intensity = Intensité
points-channel-classification = Classification
points-channel-returns = Retours
# ASPRS classes present, with point counts: "Ground (1,234), Building (567)".
label-classes = Classes
# E57 scans, with point counts.
label-scans = Scans
point-class-0 = Jamais classé
point-class-1 = Non classé
point-class-2 = Sol
point-class-3 = Végétation basse
point-class-4 = Végétation moyenne
point-class-5 = Végétation haute
point-class-6 = Bâtiment
point-class-7 = Point bas (bruit)
point-class-8 = Point clé du modèle
point-class-9 = Eau
point-class-10 = Voie ferrée
point-class-11 = Surface routière
point-class-12 = Recouvrement
point-class-13 = Câble de garde
point-class-14 = Conducteur
point-class-15 = Pylône électrique
point-class-16 = Connecteur de câble
point-class-17 = Tablier de pont
point-class-18 = Bruit haut
point-class-19 = Structure aérienne
point-class-20 = Sol ignoré
point-class-21 = Neige
point-class-22 = Exclusion temporelle
# A class without a standard name, by number.
point-class-other = Classe { $class }

## Gaussian splats

# machine-drafted, needs native review
group-splats = Splats gaussiens
label-splats = Splats
# Splats shown of those in the file, when a very large file was evenly thinned: "12,000,000 of 30,000,000".
splats-shown-of = { $shown } sur { $total }
# The degree (1 to 3) of the spherical harmonics that give each splat its view-dependent colour.
label-splat-harmonics = Couleur dépendante de la vue
splats-harmonics-none = Aucune
# $format: the file format, e.g. "SPZ".
splats-format-antialiased = { $format }, entraîné avec anticrénelage

## Avertissements

# machine-drafted, needs native review
warn-not-diagnostic = Pas pour un usage diagnostique. Specular est une visionneuse d’images généraliste, pas un dispositif médical : son rendu n’est pas étalonné pour le diagnostic.

warn-gps = Ce fichier indique où la photo a été prise. Supprimez la localisation avant de le partager si elle doit rester privée.
warn-null-island = La position GPS est exactement 0°, 0° ; l’appareil n’avait probablement pas de signal, la localisation est donc sans doute erronée.
warn-date-future = La date de prise de vue est dans le futur ; l’horloge de l’appareil était probablement mal réglée.
warn-icc-damaged = Le profil colorimétrique incorporé est endommagé et n’a pas pu être lu.
warn-metadata-failed = Les métadonnées n’ont pas pu être lues à cause d’une erreur de Specular. L’image elle-même est intacte.
warn-orientation-conflict = EXIF et XMP divergent sur l’orientation (EXIF : { $exif }, XMP : { $xmp }). La valeur EXIF est utilisée.
warn-exif-damaged = Certaines données EXIF n’ont pas pu être lues ({ $error }) ; le reste est affiché.
warn-xmp-damaged = Les métadonnées XMP sont mal formées ({ $error }) ; elles n’ont pu être lues qu’en partie.

## Fenêtre À propos

geonames-attribution = Noms de lieux : GeoNames (www.geonames.org), sous licence Creative Commons Attribution 4.0 (CC BY 4.0, https://creativecommons.org/licenses/by/4.0/). Recherche hors ligne par la crate reverse_geocoder de Grant Miner (MIT ou Apache-2.0). Les données ont été converties en une liste de villes ; elles sont fournies en l’état, sans garantie.

## TIFF and DNG structure values (All fields)
# machine-drafted, needs native review

subfile-type = { $value ->
    [0] Image en pleine résolution
    [1] Image en résolution réduite
    [2] Une page d’une image multipage
    [4] Masque de transparence
   *[other] Autre
}
predictor = { $value ->
    [1] Aucun
    [2] Différence horizontale
    [3] Différence en virgule flottante
   *[other] Autre
}
extra-samples = { $value ->
    [0] Non précisé
    [1] Alpha prémultiplié
    [2] Alpha non prémultiplié
   *[other] Autre
}
sample-format = { $value ->
    [1] Entier non signé
    [2] Entier signé
    [3] Virgule flottante
    [4] Non défini
   *[other] Autre
}

## Données géospatiales (GeoTIFF, GeoJSON, Shapefile, GeoPackage, KML/KMZ, GPX)

# machine-drafted, needs native review
group-geospatial = Géospatial
label-geo-crs = Système de coordonnées
label-geo-datum = Datum géodésique
label-geo-projection = Projection
label-geo-unit = Unités
label-geo-bounds = Emprise
label-geo-lonlat-bounds = Longitude et latitude
label-geo-pixel-size = Taille de pixel
label-geo-overviews = Aperçus
label-geo-nodata = Valeur NoData
label-geo-raster-type = Type de raster
label-geo-vertical = Référence verticale
label-geo-citation = Citation
label-geo-features = Entités
label-geo-vertices = Sommets
label-geo-layer = Couche
label-geo-layers = Couches
label-geo-tiles = Tuiles
label-geo-encoding = Encodage du texte
label-geo-warnings = Problèmes
# The companion files a georeference was read from ("scene.jgw, scene.prj").
label-geo-sidecars = Fichiers compagnons
# machine-drafted, needs native review
# A projection Specular cannot convert to longitude and latitude: $method is its name.
geo-projection-unsupported = { $method } (coordonnées non prises en charge)
# How longitude and latitude are brought to WGS 84 (what GPS and web maps use).
label-geo-to-wgs84 = Vers WGS 84
geo-to-wgs84-same = Aucune
geo-to-wgs84-helmert = Changement de datum (Helmert)
geo-to-wgs84-by-area = Changement de datum (Helmert, choisi selon la zone)
geo-to-wgs84-unknown = Inconnue : les coordonnées sont dans le datum propre au fichier
# A coordinate system's name and its EPSG code: "WGS 84 / UTM zone 33N (EPSG:32633)".
geo-crs-epsg = { $name } (EPSG:{ $code })
geo-crs-unknown = Non indiqué
# Map bounds: the lower-left and upper-right corners.
geo-bounds = { $x0 }, { $y0 } à { $x1 }, { $y1 }
# Bounds in degrees: west to east longitude, south to north latitude.
geo-lonlat-bounds = { $west }° à { $east }° de longitude, { $south }° à { $north }° de latitude
geo-pixel-size = { $x } × { $y }
geo-overviews = { $n ->
    [one] { $n } aperçu : { $sizes }
    [many] { $n } d’aperçus : { $sizes }
   *[other] { $n } aperçus : { $sizes }
}
geo-pixel-is-point = Les pixels sont des points
geo-layer = { $features ->
    [one] { $features } entité
    [many] { $features } d’entités
   *[other] { $features } entités
}, { $fields ->
    [one] { $fields } champ
    [many] { $fields } de champs
   *[other] { $fields } champs
}
# A GeoPackage tile pyramid: its table, the zoom level shown and its tile count.
geo-tiles = { $table }, zoom { $zoom }, { $tiles ->
    [one] { $tiles } tuile
    [many] { $tiles } de tuiles
   *[other] { $tiles } tuiles
}
fields-geo-geokeys = Clés GeoTIFF
fields-geo-gdal = Métadonnées GDAL
fields-geo-wkt = Système de coordonnées (WKT)
fields-geo-projjson = Système de coordonnées (PROJJSON)
# machine-drafted, needs native review
# What a web map tile set (MBTiles, PMTiles) says about itself: container, tile type, zoom
# levels, description and other metadata.
fields-geo-tileset = Jeu de tuiles
# The credit line a map's data asks to be shown with it ("© OpenStreetMap contributors").
label-geo-attribution = Crédit
# The header lines of a georeferenced raster format (ENVI, ASCII grid, DTED, ERDAS IMAGINE).
fields-geo-header = En-tête
# machine-drafted, needs native review
# GIFTI files: the format version, what the file holds, the number of data arrays and the
# encoding of their values (e.g. "GZipBase64Binary").
label-gifti-version = Version GIFTI
label-gifti-role = Contenu
gifti-role-surface = Surface
gifti-role-labels = Étiquettes par sommet
gifti-role-data = Valeurs par sommet
label-gifti-arrays = Tableaux de données
label-gifti-encoding = Encodage
label-gifti-vertices = Sommets
label-gifti-triangles = Triangles
# The brain structure, e.g. "CortexLeft".
label-gifti-structure = Structure
# A surface's geometric type, e.g. "Anatomical", "Inflated".
label-gifti-geometry = Type géométrique
# The coordinate transform of the vertices: $from and $to are coordinate spaces such as
# "scanner", "Talairach" or "MNI 152".
label-gifti-transform = Coordonnées
gifti-transform = { $from } (transformation vers { $to } inutile)
gifti-transform-applied = { $from } → { $to } (appliquée)
# The surface file a data file is drawn on, found beside it.
label-gifti-surface = Affiché sur
# The data array shown: its name, NIfTI intent (e.g. "NIFTI_INTENT_SHAPE") and data type.
label-gifti-array = Tableau
label-gifti-intent = Intent
label-gifti-data-type = Type de données
# Entries of the label table (names and colours of regions).
label-gifti-labels = Table des étiquettes
# CIFTI-2 files: the version, NIfTI intent (e.g. "ConnDenseScalar") and matrix size.
label-cifti-version = Version CIFTI
label-cifti-intent = Intent
label-cifti-matrix = Matrice
cifti-matrix = { $rows } × { $columns }
# The maps (pages) of the file and what they are.
label-cifti-maps = Cartes
cifti-maps-scalars = { $count ->
        [one] { $count } carte scalaire
       *[other] { $count } cartes scalaires
    }
cifti-maps-labels = { $count ->
        [one] { $count } carte d’étiquettes
       *[other] { $count } cartes d’étiquettes
    }
cifti-maps-series = { $count ->
        [one] { $count } instant
       *[other] { $count } instants
    }
cifti-maps-parcels = { $count ->
        [one] { $count } parcelle
       *[other] { $count } parcelles
    }
cifti-maps-other = { $count }
# machine-drafted, needs native review
# A dense connectivity file (.dconn.nii): every brain place's connectivity to every other.
# $seeds: how many places can be picked; $size: the bytes read when one is picked (e.g. "357 KB").
label-cifti-dense = Connectivité
cifti-dense = { $seeds ->
        [one] { $seeds } graine
       *[other] { $seeds } graines
    }, { $size } lus par graine
# A time series' start and step; $unit is e.g. "second".
label-cifti-series = Série
cifti-series = à partir de { $start }, tous les { $step } ({ $unit })
label-cifti-models = Modèles cérébraux
label-cifti-parcels = Parcelles
label-cifti-structures = Structures
label-cifti-voxels = Voxels sous-corticaux
# A surface the file's values lie on: the brain structure (e.g. "CORTEX_LEFT"), its vertex
# count, and the surface file found beside the file for it, or that none was found.
label-cifti-surface = Surface
cifti-surface-found = { $structure } : { $vertices ->
        [one] { $vertices } sommet
       *[other] { $vertices } sommets
    }, { $file }
cifti-surface-missing = { $structure } : nécessite une surface de { $vertices ->
        [one] { $vertices } sommet
       *[other] { $vertices } sommets
    } (aucune à côté du fichier)
# How the file is shown: on the surfaces found beside it, or as its subcortical voxels.
label-cifti-shown = Affiché comme
cifti-shown-surfaces = Surfaces colorées
cifti-shown-voxels = Volume des voxels sous-corticaux
# machine-drafted, needs native review
# Shown with a picked seed's row: click a surface vertex, or move the crosshair in MPR.
cifti-shown-connectivity = Connectivité de la graine choisie
# Maps left out of the voxel volume, which would be too large with them.
label-cifti-left-out = Cartes omises
# Tractograms: format, number of streamlines (fibre tracts) and points.
label-tract-format = Format
label-tract-streamlines = Lignes de courant
# A large tractogram draws only some of its streamlines.
tract-streamlines-shown = { $count } ({ $shown } affichées)
label-tract-points = Points
label-tract-mean-length = Longueur moyenne
# A length in millimetres.
tract-mm = { $value } mm
# Values per point and per streamline the file stores, by name.
label-tract-scalars = Scalaires par point
label-tract-properties = Propriétés par ligne de courant
# TrackVis: the voxel grid the tracts were made in, its voxel size and voxel order (e.g. "LAS").
label-tract-grid = Grille
label-tract-voxel-size = Taille de voxel
label-tract-voxel-order = Ordre des voxels
# How the points were brought into scanner (RAS) millimetres.
label-tract-space = Coordonnées
tract-space-vox-to-ras = D’après la matrice voxel vers RAS
tract-space-voxel-order = D’après l’ordre des voxels (pas de matrice voxel vers RAS)
tract-space-scanner = Espace scanner
# MRtrix: the tracking step size.
label-tract-step = Pas

## File headers (DPX, Cineon, XWD, MIFF, TIM, WSQ, JBIG)

# machine-drafted, needs native review
# What the file's own header records, for formats without Exif or XMP.
group-header = En-tête du fichier
label-header-created = Création
# The program or person that wrote the file.
label-header-creator = Créateur
label-header-copyright = Copyright
label-header-project = Projet
label-header-comment = Commentaire
# The scanner, camera or other device the image came from.
label-header-device = Appareil
# SMPTE time code of the frame: "01:00:00:12".
label-header-timecode = Timecode
# A film frame's edge code (KeyKode).
label-header-edge-code = Code de bord
label-header-frame-rate = Fréquence d’images
# Where the frame sits in its sequence.
label-header-frame = Trame
# Film stock and slate.
label-header-film = Film
# How code values relate to light: printing density, linear, BT.709…
label-header-transfer = Fonction de transfert
# The colour primaries the file states.
label-header-colorimetric = Colorimétrie
# The X window a screen dump was taken of.
label-header-window = Fenêtre
label-header-resolution = Résolution
label-header-compression = Compression
# machine-drafted, needs native review
# JPEG XS: the profile and the level the codestream states ("Main 422.10"; "4k-1, sublevel 6 bpp").
label-header-profile = Profil
# machine-drafted, needs native review
label-header-level = Niveau
# machine-drafted, needs native review
# JPEG XS: how many wavelet levels, horizontal and vertical ("5 horizontal, 1 vertical").
label-header-decomposition = Niveaux d’ondelettes
# machine-drafted, needs native review
# JPEG XS: the colour transform ("None", "Reversible colour transform (RCT)", "Star-Tetrix, RGGB mosaic").
label-header-colour-transform = Transformation des couleurs
# machine-drafted, needs native review
# Photoshop: the artboards, by name and size ("Phone (375 × 667), Tablet (768 × 1024)").
label-header-artboards = Plans de travail
# machine-drafted, needs native review
# Photoshop file without a merged image, shown from its layers: the features it leaves out,
# by Photoshop's names ("Color Lookup, Knockout").
label-header-not-drawn = Non dessiné
# machine-drafted, needs native review
# The same, for the features drawn only approximately ("Bevel & Emboss, Satin").
label-header-approximated = Dessiné de façon approchée

## Fonts (TTF, OTF, TTC/OTC, WOFF, WOFF2)

# machine-drafted, needs native review
group-font = Police
label-font-family = Famille
label-font-style = Style
# The face's place in a font collection: "2 of 4".
label-font-face = Face
font-face-of = { $index } sur { $count }
label-font-version = Version
label-font-designer = Dessinateur
# The font's maker (the name table's manufacturer).
label-font-manufacturer = Fonderie
label-font-glyphs = Glyphes
label-font-outlines = Contours
font-outlines-truetype = TrueType (quadratiques)
font-outlines-cff = PostScript (CFF, cubiques)
font-outlines-cff2 = PostScript (CFF2, cubiques)
font-outlines-bitmap = Bitmaps seulement
font-outlines-none = Aucun
# What the font's licence flags (OS/2 fsType) allow documents that embed it to do.
label-font-embedding = Incorporation
font-embedding-installable = Peut être incorporée et installée.
font-embedding-restricted = Ne peut pas être incorporée.
font-embedding-preview-print = Peut être incorporée pour l’affichage et l’impression seulement.
font-embedding-editable = Peut être incorporée dans des documents modifiables.
font-embedding-no-subsetting = Entière seulement, pas en sous-ensemble.
font-embedding-bitmap-only = Seulement en bitmaps.
# Variation axes of a variable font: "Weight 100–900 (400)".
label-font-axes = Axes de variation
font-axis = { $name } { $min }–{ $max } ({ $default })
label-font-instances = Styles nommés
label-font-colour = Glyphes en couleur
font-colour-colr0 = Calques COLR
font-colour-colr1 = Dégradés COLR v1
font-colour-sbix = Bitmaps (sbix)
font-colour-cbdt = Bitmaps (CBDT)
font-colour-svg = SVG
label-font-weight = Graisse
label-font-units-per-em = Unités par em
label-font-tables = Tables
label-font-licence = Licence
label-font-licence-url = URL de la licence
label-font-copyright = Copyright
label-font-trademark = Marque

## Office documents (Word, Excel, PowerPoint, iWork, OpenDocument)

# machine-drafted, needs native review
group-document = Document
label-document-kind = Type
document-kind-word = Document Word
document-kind-excel = Classeur Excel
document-kind-powerpoint = Présentation PowerPoint
document-kind-pages = Document Pages
document-kind-numbers = Feuille de calcul Numbers
document-kind-keynote = Présentation Keynote
document-kind-odf-text = Texte OpenDocument
document-kind-odf-spreadsheet = Classeur OpenDocument
document-kind-odf-presentation = Présentation OpenDocument
document-kind-odf-graphics = Dessin OpenDocument
document-kind-odf-other = Fichier OpenDocument
# machine-drafted, needs native review
document-kind-playlist = Playlist
# machine-drafted, needs native review
label-playlist-items = Éléments
# Appended to the kind: "Word document, template, with macros".
document-template = modèle
document-macros = avec macros
document-legacy = format Office 97–2003
label-document-preview = Aperçu
# The image format of the preview stored in the file, e.g. "JPEG image".
document-preview-image = Image { $format }
document-preview-none = Aucun dans ce fichier
label-document-subject = Objet
label-document-author = Auteur
label-document-description = Description
label-document-category = Catégorie
label-document-last-saved-by = Dernier enregistrement par
label-document-created = Créé
label-document-modified = Modifié
label-document-printed = Dernière impression
label-document-pages = Pages
label-document-slides = Diapositives
label-document-hidden-slides = Diapositives masquées
label-document-notes = Pages de commentaires
label-document-sheets = Feuilles
label-document-words = Mots
label-document-characters = Caractères
label-document-paragraphs = Paragraphes
label-document-lines = Lignes
label-document-editing-time = Temps d’édition
document-minutes = { $minutes } min
label-document-revision = Révision
label-document-application = Enregistré avec
label-document-company = Société
label-document-template = Modèle

## Books (EPUB)

# machine-drafted, needs native review
group-book = Livre
label-book-authors = Auteurs
label-book-publisher = Éditeur
label-book-date = Publié
label-book-language = Langue
label-book-pages = Pages
# Entries in the book's table of contents.
label-book-contents = Entrées de la table des matières
label-book-layout = Mise en page
book-layout-reflowable = Redimensionnable
book-layout-fixed = Mise en page fixe
book-right-to-left = de droite à gauche
label-book-version = Format
book-version = EPUB { $version }
label-book-fonts = Polices incorporées
book-fonts-obfuscated = { $count } ({ $obfuscated } obscurcies)
label-book-subjects = Sujets
label-book-identifier = Identifiant
label-book-rights = Droits

unit-symbol-mm = mm
unit-symbol-cm = cm
unit-symbol-m = m
unit-symbol-km = km
unit-symbol-mm2 = mm²
unit-symbol-cm2 = cm²
unit-symbol-m2 = m²
unit-symbol-km2 = km²
unit-symbol-ha = ha
unit-symbol-ft = ft
unit-symbol-ft2 = ft²
unit-symbol-mi = mi
unit-symbol-mi2 = mi²
unit-symbol-ac = ac

# machine-drafted, needs native review
volume-orient-parrec = Angulation et décentrage Philips
volume-orient-ecat = Tailles de voxel et décalages ECAT (peu fiables)
label-volume-technique = Technique
label-volume-version = Version du format
label-volume-isotope = Isotope
label-volume-tracer = Traceur
label-volume-units = Unités

# Text, tables and subtitles.
label-header-encoding = Jeu de caractères
label-header-lines = Lignes
label-header-line-ends = Fins de ligne
label-header-words = Mots
label-header-pages = Pages
label-header-pictures = Images
label-header-pictures-not-shown = Images non affichées
label-header-tables = Tableaux
label-header-rows = Lignes du tableau
label-header-columns = Colonnes
label-header-delimiter = Séparateur
label-header-column-names = Noms des colonnes
label-header-cues = Sous-titres
label-header-cue-span = Sous-titres de – à
