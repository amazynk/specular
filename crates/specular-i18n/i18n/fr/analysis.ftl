# Specular — Medical analysis: segmentations, SUV, fusion, TMTV, time-intensity curves. Français. machine-drafted, needs native review

palette-segments = Afficher ou masquer les segmentations
palette-segment-opacity = Opacité de remplissage des segmentations suivante
palette-fusion = Fusionner les séries du même référentiel
palette-fusion-colormap = Palette de fusion suivante
palette-fusion-blend = Opacité de fusion suivante
palette-fusion-nudge = Déplacer la série fusionnée à la main
palette-tmtv = Outil TMTV (sphère, boîte, désactivé)
palette-tmtv-threshold = Seuil TMTV (SUV { NUMBER(2.5) } ou { PERCENT(41) } du SUVmax)
palette-time-curve = Courbe temps-intensité de la dernière région
palette-export-curve = Enregistrer la courbe temps-intensité en CSV

analysis-searching = Recherche de segmentations et de séries à fusionner en cours
segments-none = Aucune segmentation de ce référentiel n’a été trouvée
segments-shown = Segmentations affichées
segments-hidden = Segmentations masquées
segments-fill = Remplissage des segmentations { PERCENT($percent) }
segments-failed = Segmentation non affichée : { $reason }
segment-entry = { $name } · { $ml } mL

suv-value = SUVbw { $value }
suv-factor = Facteur SUVbw { $factor }, { $minutes } min après l’injection
suv-refused = Pas de SUV : { $reason }
suv-preset = SUV 0–{ $upper }
suv-refused-not-pet = pas une image PET
suv-refused-units = les unités des pixels ne sont pas en Bq/mL
suv-refused-corrected = pas de correction d’atténuation et de décroissance
suv-refused-decay = correction de décroissance non prise en charge
suv-refused-weight = pas de poids du patient
suv-refused-dose = pas de dose injectée
suv-refused-half-life = pas de demi-vie du radionucléide
suv-refused-injection = pas d’heure d’injection
suv-refused-scan-time = pas d’heure d’acquisition
suv-refused-delay = l’injection est postérieure à l’examen
suv-refused-scale = pas de facteur d’échelle SUV Philips
suv-refused-height = pas de taille du patient
suv-refused-sex = pas de sexe du patient
suv-refused-lbm = masse maigre hors limites

colormap-hot-iron = Fer chaud
colormap-rainbow = Arc-en-ciel
colormap-viridis = Viridis
fusion-none = Aucune série PET, SPECT, CT ou MR de ce référentiel à fusionner
fusion-needed = Fusionnez d’abord une série (Shift+U)
fusion-opening = Fusion de { $modality }
fusion-off = Fusion désactivée
fusion-refused = Fusion impossible : { $reason }
fusion-on = { $modality } fusionné : { $colormap }, { PERCENT($blend) }
fusion-hidden = { $modality } fusionné masqué
fusion-window-suv = Fenêtre fusionnée SUV { $low }–{ $high }
fusion-value = { $modality } { $value }
fusion-value-suv = { $modality } SUVbw { $value }
fusion-blend-toast = Opacité de fusion { PERCENT($percent) }
fusion-moved = Déplacé à la main : { $x }, { $y }, { $z } mm, { $degrees }°
fusion-nudge-hint = Les flèches déplacent la série fusionnée de 1 mm, Shift+Up/Down le long de la vue, [ et ] la font tourner
fusion-nudge-off = Série fusionnée fixée

tmtv-hint-sphere = TMTV : faites glisser une sphère depuis son centre
tmtv-hint-box = TMTV : faites glisser une boîte
tmtv-off = TMTV désactivé
tmtv-threshold-suv = Seuil SUV { $suv }
tmtv-threshold-share = Seuil { PERCENT($share) } du SUVmax
tmtv-result = TMTV { $ml } mL dans { $count ->
        [one] { $count } région
       *[other] { $count } régions
    }
tmtv-stats = SUVmax { $max }, SUVmean { $mean }, TLG { $tlg }
tmtv-outside = La région est hors du volume
tmtv-no-pet = Le TMTV nécessite un volume PET avec SUV

analysis-mpr-only = La fusion et le TMTV fonctionnent en MPR (Shift+R)
curve-not-dynamic = Ni une image multi-images ni une série dynamique
curve-not-mpr = Les courbes temps-intensité se tracent sur l’image, hors MPR
curve-no-roi = Tracez d’abord une ellipse ou un rectangle
curve-loading = { $count ->
        [one] Lecture de { $count } instant…
       *[other] Lecture de { $count } instants…
    }
curve-title = Courbe temps-intensité
curve-mean = Moyenne
curve-max = Max
curve-seconds = s
curve-saved = Courbe enregistrée
curve-none = Aucune courbe à enregistrer
curve-failed = Courbe non créée : { $reason }
