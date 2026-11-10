# Specular — File kinds: which format a shared extension is, Open as…, remembered choices. Français.

## Which kind of file? Files whose extension several formats share.

# machine-drafted, needs native review
file-kind-title = Quel type de fichier est { $file } ?
# machine-drafted, needs native review
file-kind-why-content = Son contenu correspond à plusieurs formats.
# machine-drafted, needs native review
file-kind-why-name = Son contenu ne le dit pas. Ces formats utilisent des fichiers .{ $ext }.
# machine-drafted, needs native review
file-kind-why-none = Aucun format ne reconnaît son contenu.
# machine-drafted, needs native review
file-kind-certain = Certain
# machine-drafted, needs native review
file-kind-likely = Probable
# machine-drafted, needs native review
file-kind-possible = Possible
# machine-drafted, needs native review
file-kind-guess = D’après le nom seulement
# machine-drafted, needs native review
file-kind-hint-same = comme les autres fichiers .{ $ext } ici
# machine-drafted, needs native review
file-kind-hint-companions = ses fichiers habituels sont à côté
# machine-drafted, needs native review
file-kind-scope = L’utiliser pour
# machine-drafted, needs native review
file-kind-scope-file = Ce fichier
# machine-drafted, needs native review
file-kind-scope-folder = Fichiers .{ $ext } de ce dossier
# machine-drafted, needs native review
file-kind-scope-everywhere = Tous les fichiers .{ $ext }
# machine-drafted, needs native review
file-kind-later = Modifiable plus tard avec Ouvrir comme… dans le menu du fichier.
# machine-drafted, needs native review
file-kind-open = Ouvrir
# machine-drafted, needs native review
file-kind-raw = Échantillons bruts
# machine-drafted, needs native review
file-kind-open-as-none = Rien d’autre ne lit { $file }.
# machine-drafted, needs native review
error-reason-ambiguous = Il peut s’agir de plusieurs formats. Choisissez-en un pour l’ouvrir.

## Open as… (the palette and the gallery's file menu)

# machine-drafted, needs native review
palette-open-as = Ouvrir comme…
# machine-drafted, needs native review
palette-open-as-help = Choisir le format dans lequel lire le fichier.

## Settings › Optional decoders › File kinds

# machine-drafted, needs native review
settings-decoders-file-kinds = Oublier tous les types de fichiers…
# machine-drafted, needs native review
settings-decoders-file-kinds-help = Formats choisis pour des extensions que partagent plusieurs formats. La question sera reposée la prochaine fois.
# machine-drafted, needs native review
file-kind-rules-intro = Chaque règle ouvre les fichiers d’une extension dans un format.
# machine-drafted, needs native review
file-kind-rules-none = Aucune règle pour l’instant. Choisissez « Fichiers .ext de ce dossier » ou « Tous les fichiers .ext » quand la question est posée.
# machine-drafted, needs native review
file-kind-rule = Fichiers .{ $ext } : { $format }
# machine-drafted, needs native review
file-kind-rule-folder = Dans { $folder }
# machine-drafted, needs native review
file-kind-rule-everywhere = Partout
# machine-drafted, needs native review
file-kind-forget = Oublier…
# machine-drafted, needs native review
file-kind-forget-ask = Oublier « Fichiers .{ $ext } : { $format } » ?
# machine-drafted, needs native review
file-kind-forget-detail = { -app-name } reposera la question la prochaine fois. Aucun fichier n’est modifié ni supprimé.
# machine-drafted, needs native review
file-kind-forget-ok = Oublier
# machine-drafted, needs native review
file-kind-forgotten = Oublié : fichiers .{ $ext } : { $format }.
# machine-drafted, needs native review
file-kind-forget-all-ask = { $count ->
    [one] Oublier la règle de type de fichier ?
   *[other] Oublier les { $count } règles de type de fichier ?
}
# machine-drafted, needs native review
file-kind-forgotten-all = { $count ->
    [one] Règle de type de fichier oubliée.
   *[other] { $count } règles de type de fichier oubliées.
}
# machine-drafted, needs native review
file-kind-keywords = extension, format, ouvrir comme, quel type, type de fichier

## One line on what each format is, under its name in the prompt.

# machine-drafted, needs native review
file-kind-about-other = Un autre format qui utilise des fichiers .{ $ext }.
# machine-drafted, needs native review
file-kind-about-raw-volume = Échantillons sans en-tête. Vous indiquez largeur, hauteur, coupes et type d’échantillon.
# machine-drafted, needs native review
file-kind-about-raw = Photo brute d’appareil.
# machine-drafted, needs native review
file-kind-about-flir-fff = Image thermique FLIR avec températures.
# machine-drafted, needs native review
file-kind-about-fits = Image astronomique (FITS).
# machine-drafted, needs native review
file-kind-about-fit = Trace d’activité Garmin : points GPS et données des capteurs.
# machine-drafted, needs native review
file-kind-about-mif = Volume médical MRtrix (IRM).
# machine-drafted, needs native review
file-kind-about-map-info-mif = Carte vectorielle MapInfo, avec ses données .mid.
# machine-drafted, needs native review
file-kind-about-net-cdf = Données scientifiques en grille (climat, océan).
# machine-drafted, needs native review
file-kind-about-hdf5 = Données scientifiques HDF5.
# machine-drafted, needs native review
file-kind-about-playlist = Liste de lecture audio (PLS).
# machine-drafted, needs native review
file-kind-about-gerber = Couche de circuit imprimé (Gerber), comme la pâte à braser.
# machine-drafted, needs native review
file-kind-about-ser = Vidéo astronomique d’une caméra planétaire (SER).
# machine-drafted, needs native review
file-kind-about-tia = Série de microscope électronique (FEI TIA).
file-kind-about-pcd = Nuage de points 3D (PCD).
file-kind-about-photo-cd = Numérisation Kodak Photo CD.
file-kind-about-zx-screen = Copie d’écran ZX Spectrum.
file-kind-about-cpc-screen = Copie d’écran Amstrad CPC.
file-kind-about-msx-screen = Copie d’écran MSX.
file-kind-about-scc = Sous-titres codés Scenarist.
# machine-drafted, needs native review
file-kind-about-pict = Image Mac classique (PICT).
# machine-drafted, needs native review
file-kind-about-hdr = Image HDR Radiance.
# machine-drafted, needs native review
file-kind-about-analyze = Volume médical Analyze 7.5, avec son .hdr.
# machine-drafted, needs native review
file-kind-about-envi = Raster de télédétection (ENVI), avec son .hdr.
# machine-drafted, needs native review
file-kind-about-pds3 = Image de mission planétaire (PDS3), avec son étiquette.
# machine-drafted, needs native review
file-kind-about-mpeg-ps = Vidéo de caméscope (flux programme MPEG, MOD/TOD).
# machine-drafted, needs native review
file-kind-about-tracker = Module musical tracker (MOD, S3M, XM, IT, 669, MTM, STM, OKT, MED, ULT, FAR).
# machine-drafted, needs native review
file-kind-about-softimage-pic = Rendu 3D Softimage (PIC).
# machine-drafted, needs native review
file-kind-about-dr-halo = Image Dr. Halo, avec sa palette .pal.
# machine-drafted, needs native review
file-kind-about-wal = Texture Quake 2, colorée par la palette du jeu.
# machine-drafted, needs native review
file-kind-about-quick-time = Film QuickTime.
file-kind-about-pro-res-raw = Vidéo Apple ProRes RAW : images du capteur de la caméra.
file-kind-about-cinema-raw-light = Vidéo Canon Cinema RAW Light : images du capteur de la caméra.
# machine-drafted, needs native review
file-kind-about-gimp-brush = Brosse GIMP.
# machine-drafted, needs native review
file-kind-about-swatch-exchange = Palette de couleurs Adobe Swatch Exchange.
# machine-drafted, needs native review
file-kind-about-aseprite = Sprite pixel art Aseprite.
