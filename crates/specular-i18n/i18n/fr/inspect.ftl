# machine-drafted, needs native review
# Outils d’inspection : écrêtage, focus peaking, histogramme, inspecteur de pixels.

## Palette de commandes
palette-clipping = Afficher ou masquer les alertes d’écrêtage
palette-focus-peaking = Afficher ou masquer le focus peaking
palette-histogram = Afficher ou masquer l’histogramme
palette-pixel-inspector = Afficher ou masquer l’inspecteur de pixels
# machine-drafted, needs native review
palette-crosshair = Réticule clavier
palette-announce-cursor = Lire la valeur sous le curseur

## Histogramme
histogram-title = Histogramme
histogram-clipped = Écrêtage : hautes lumières { $high }, ombres { $low }
histogram-empty = Aucune image visible
histogram-a11y = Histogramme de l’image visible : luminosité moyenne { $mean }, hautes lumières écrêtées { $high }, ombres écrêtées { $low }

## Inspecteur de pixels
inspector-title = Inspecteur de pixels
inspector-position = Position
inspector-source-rgb = Source RVB
inspector-source-rgba = Source RVBA
inspector-source-ycc = Source Y′CbCr
inspector-source-gray = Source gris
inspector-source-gray16 = Source gris (16 bits)
inspector-source-gray-alpha16 = Source gris + alpha (16 bits)
inspector-source-rgb16 = Source RVB (16 bits)
inspector-source-rgba16 = Source RVBA (16 bits)
inspector-source-pending = Source
inspector-srgb = sRVB
inspector-hex = Hex
inspector-linear = Linéaire
inspector-approximate = D’après un aperçu réduit ; zoomez pour des valeurs exactes
inspector-outside = Pointez l’image pour inspecter un pixel
inspector-a11y = Pixel en { $x }, { $y } : sRVB { $value }

## Réglages
settings-section-inspection = Inspection
settings-inspection-clip-highlight = Seuil d’écrêtage des hautes lumières
settings-inspection-clip-highlight-help = Les pixels dont un canal atteint ou dépasse cette valeur 8 bits sont marqués comme hautes lumières écrêtées.
settings-inspection-clip-shadow = Seuil d’écrêtage des ombres
settings-inspection-clip-shadow-help = Les pixels dont un canal atteint cette valeur 8 bits ou moins sont marqués comme ombres écrêtées.
settings-inspection-highlight-color = Couleur d’alerte des hautes lumières
settings-inspection-highlight-color-help = Couleur qui marque les hautes lumières écrêtées.
settings-inspection-shadow-color = Couleur d’alerte des ombres
settings-inspection-shadow-color-help = Couleur qui marque les ombres écrêtées.
settings-inspection-peaking-sensitivity = Sensibilité du focus peaking
settings-inspection-peaking-sensitivity-help = Des valeurs plus élevées marquent aussi les bords plus doux comme nets.
settings-inspection-peaking-color = Couleur du focus peaking
settings-inspection-peaking-color-help = Couleur qui marque les bords nets.
settings-inspection-histogram-channels = Canaux de l’histogramme
settings-inspection-histogram-channels-help = Les courbes affichées par l’histogramme.
settings-inspection-histogram-channels-colour = Rouge, vert et bleu
settings-inspection-histogram-channels-luminance = Luminance
settings-inspection-histogram-channels-both = Couleur et luminance
settings-inspection-histogram-log = Histogramme logarithmique
settings-inspection-histogram-log-help = Mettre l’histogramme à l’échelle logarithmique pour garder visibles les petites valeurs.

## Channel isolation (machine-drafted, needs native review)
palette-channel-red = Afficher le canal rouge
palette-channel-green = Afficher le canal vert
palette-channel-blue = Afficher le canal bleu
palette-channel-alpha = Afficher le canal alpha (à nouveau : sur damier, puis sur une couleur)
palette-channel-luma = Afficher la luminance
channel-normal = Tous les canaux
channel-red = Canal rouge
channel-green = Canal vert
channel-blue = Canal bleu
channel-alpha = Canal alpha
channel-alpha-checker = Transparence sur damier
channel-alpha-matte = Transparence sur couleur
channel-luma = Luminance
disabled-no-alpha = Cette image n’a pas de transparence
settings-inspection-alpha-matte-color = Couleur de fond alpha
settings-inspection-alpha-matte-color-help = Couleur unie derrière l’image dans la vue « Transparence sur couleur ».

## Colour-vision-safe display: false colour, colour bars, channel colours, CVD simulation, HiLo
palette-false-colour = Afficher ou masquer les fausses couleurs
palette-next-colour-map = Palette de couleurs suivante
palette-channel-colours = Couleurs de canaux suivantes (rouge–vert, magenta–vert, magenta–jaune–cyan)
palette-hilo = Afficher ou masquer HiLo (pixels au maximum et au minimum de la profondeur de bits)
palette-colour-vision = Simulation de vision des couleurs suivante
palette-colour-vision-off = Arrêter la simulation de vision des couleurs
palette-colour-vision-protanopia = Simuler la protanopie (sans cônes rouges)
palette-colour-vision-deuteranopia = Simuler la deutéranopie (sans cônes verts)
palette-colour-vision-tritanopia = Simuler la tritanopie (sans cônes bleus)
palette-colour-vision-achromatopsia = Simuler l’achromatopsie (sans couleurs)
# Colour map names. Proper names of published maps; usually left untranslated.
colour-map-viridis = Viridis
colour-map-cividis = Cividis
colour-map-batlow = Batlow
colour-map-roma = Roma
colour-map-vik = Vik
colour-map-oslo = Oslo
colour-map-lajolla = Lajolla
colour-map-thermal = Thermal
colour-map-haline = Haline
colour-map-solar = Solar
colour-map-ice = Ice
colour-map-deep = Deep
colour-map-dense = Dense
colour-map-algae = Algae
colour-map-rain = Rain
colour-map-speed = Speed
colour-map-balance = Balance
colour-map-curl = Curl
colour-map-phase = Phase
colour-map-jet = Jet (non adapté aux daltoniens)
# The colour bar's title. $map is a colour map name, $unit the value unit (HU, counts…).
colour-bar-title = { $map }
colour-bar-title-unit = { $map } · { $unit }
# Unit of raw sample values without a physical unit.
colour-bar-unit-raw = valeurs brutes
# Screen reader summary of the colour bar.
colour-bar-a11y = Barre de couleurs { $map } : de { $low } à { $high } { $unit }
# Badges at the top of the window while a display aid is on (also announced).
badge-false-colour = Fausses couleurs : { $map }
badge-hilo = HiLo : pixels au maximum et au minimum marqués
badge-colour-vision-protanopia = Simulation de protanopie
badge-colour-vision-deuteranopia = Simulation de deutéranopie
badge-colour-vision-tritanopia = Simulation de tritanopie
badge-colour-vision-achromatopsia = Simulation d’achromatopsie
channel-colours-red-green = Canaux : rouge, vert, bleu
channel-colours-magenta-green = Canaux : magenta, vert
channel-colours-magenta-yellow-cyan = Canaux : magenta, jaune, cyan
settings-inspection-false-colour-map = Palette de fausses couleurs
settings-inspection-false-colour-map-help = Palette des fausses couleurs et de leur barre de couleurs. Toutes sauf jet et phase restent lisibles quelle que soit la vision des couleurs.
settings-inspection-false-colour-map-viridis = Viridis
settings-inspection-false-colour-map-cividis = Cividis
settings-inspection-false-colour-map-batlow = Batlow
settings-inspection-false-colour-map-roma = Roma (divergente)
settings-inspection-false-colour-map-vik = Vik (divergente)
settings-inspection-false-colour-map-oslo = Oslo
settings-inspection-false-colour-map-lajolla = Lajolla
settings-inspection-false-colour-map-thermal = Thermal
settings-inspection-false-colour-map-haline = Haline
settings-inspection-false-colour-map-solar = Solar
settings-inspection-false-colour-map-ice = Ice
settings-inspection-false-colour-map-deep = Deep
settings-inspection-false-colour-map-dense = Dense
settings-inspection-false-colour-map-algae = Algae
settings-inspection-false-colour-map-rain = Rain
settings-inspection-false-colour-map-speed = Speed
settings-inspection-false-colour-map-balance = Balance (divergente)
settings-inspection-false-colour-map-curl = Curl (divergente)
settings-inspection-false-colour-map-phase = Phase (cyclique)
settings-inspection-false-colour-map-jet = Jet (non adapté aux daltoniens)
settings-inspection-colour-bar = Barre de couleurs
settings-inspection-colour-bar-help = Afficher une barre de couleurs avec les valeurs dans les unités propres de l’image quand les fausses couleurs sont actives.
settings-inspection-channel-colours = Couleurs des canaux
settings-inspection-channel-colours-help = Comment les canaux de couleur sont affichés. Magenta–vert et magenta–jaune–cyan restent distincts en cas de daltonisme rouge–vert.
settings-inspection-channel-colours-red-green = Rouge, vert, bleu
settings-inspection-channel-colours-magenta-green = Magenta, vert
settings-inspection-channel-colours-magenta-yellow-cyan = Magenta, jaune, cyan
settings-inspection-colour-vision = Simulation de vision des couleurs
settings-inspection-colour-vision-help = Afficher toute la fenêtre telle qu’elle est vue avec un trouble de la vision des couleurs (Machado et al. 2009). Les exports ne sont jamais affectés.
settings-inspection-colour-vision-off = Désactivée
settings-inspection-colour-vision-protanopia = Protanopie
settings-inspection-colour-vision-deuteranopia = Deutéranopie
settings-inspection-colour-vision-tritanopia = Tritanopie
settings-inspection-colour-vision-achromatopsia = Achromatopsie
settings-inspection-auto-range = Plage auto pour les gris haute profondeur
settings-inspection-auto-range-help = Étirer les images en niveaux de gris 16 bits (TIFF, PNG, FITS) de leur valeur la plus sombre à la plus claire. Les images DICOM gardent leur propre fenêtre.

## Keyboard crosshair and the value under the cursor (machine-drafted, needs native review)

crosshair-on = Réticule activé. Les flèches le déplacent, Maj pour des pas plus grands. Entrée clique. Échap termine.
crosshair-off = Réticule désactivé
announce-cursor-nothing = Rien sous le curseur
