# Specular — Medical analysis: segmentations, SUV, fusion, TMTV, time-intensity curves. Español. machine-drafted, needs native review

palette-segments = Mostrar u ocultar segmentaciones
palette-segment-opacity = Siguiente opacidad de relleno de segmentación
palette-fusion = Fusionar las series del mismo marco de referencia
palette-fusion-colormap = Siguiente mapa de color de fusión
palette-fusion-blend = Siguiente opacidad de fusión
palette-fusion-nudge = Mover a mano la serie fusionada
palette-tmtv = Herramienta TMTV (esfera, caja, desactivada)
palette-tmtv-threshold = Umbral TMTV (SUV { NUMBER(2.5) } o { PERCENT(41) } del SUVmax)
palette-time-curve = Curva tiempo-intensidad de la última región
palette-export-curve = Guardar la curva tiempo-intensidad como CSV

analysis-searching = Aún buscando segmentaciones y series para fusionar
segments-none = No se encontró ninguna segmentación de este marco de referencia
segments-shown = Segmentaciones visibles
segments-hidden = Segmentaciones ocultas
segments-fill = Relleno de segmentación { PERCENT($percent) }
segments-failed = Segmentación no mostrada: { $reason }
segment-entry = { $name } · { $ml } mL

suv-value = SUVbw { $value }
suv-factor = Factor SUVbw { $factor }, { $minutes } min tras la inyección
suv-refused = Sin SUV: { $reason }
suv-preset = SUV 0–{ $upper }
suv-refused-not-pet = no es una imagen PET
suv-refused-units = las unidades de píxel no son Bq/mL
suv-refused-corrected = sin corrección de atenuación y decaimiento
suv-refused-decay = corrección de decaimiento no compatible
suv-refused-weight = sin peso del paciente
suv-refused-dose = sin dosis inyectada
suv-refused-half-life = sin semivida del radionúclido
suv-refused-injection = sin hora de inyección
suv-refused-scan-time = sin hora de adquisición
suv-refused-delay = la inyección es posterior a la adquisición
suv-refused-scale = sin factor de escala SUV de Philips
suv-refused-height = sin altura del paciente
suv-refused-sex = sin sexo del paciente
suv-refused-lbm = masa magra fuera de rango

colormap-hot-iron = Hierro candente
colormap-rainbow = Arcoíris
colormap-viridis = Viridis
fusion-none = No hay series PET, SPECT, CT o MR de este marco de referencia para fusionar
fusion-needed = Fusione antes una serie (Shift+U)
fusion-opening = Fusionando { $modality }
fusion-off = Fusión desactivada
fusion-refused = No se puede fusionar: { $reason }
fusion-on = { $modality } fusionada: { $colormap }, { PERCENT($blend) }
fusion-hidden = { $modality } fusionada oculta
fusion-window-suv = Ventana de fusión SUV { $low }–{ $high }
fusion-value = { $modality } { $value }
fusion-value-suv = { $modality } SUVbw { $value }
fusion-blend-toast = Opacidad de fusión { PERCENT($percent) }
fusion-moved = Movida a mano: { $x }, { $y }, { $z } mm, { $degrees }°
fusion-nudge-hint = Las flechas mueven la serie fusionada 1 mm, Shift+Up/Down a lo largo de la vista, [ y ] la giran
fusion-nudge-off = Serie fusionada fijada

tmtv-hint-sphere = TMTV: arrastre una esfera desde su centro
tmtv-hint-box = TMTV: arrastre una caja
tmtv-off = TMTV desactivado
tmtv-threshold-suv = Umbral SUV { $suv }
tmtv-threshold-share = Umbral { PERCENT($share) } del SUVmax
tmtv-result = TMTV { $ml } mL en { $count ->
        [one] { $count } región
       *[other] { $count } regiones
    }
tmtv-stats = SUVmax { $max }, SUVmean { $mean }, TLG { $tlg }
tmtv-outside = La región está fuera del volumen
tmtv-no-pet = TMTV necesita un volumen PET con SUV

analysis-mpr-only = La fusión y TMTV funcionan en MPR (Shift+R)
curve-not-dynamic = No es una imagen multifotograma ni una serie dinámica
curve-not-mpr = Las curvas tiempo-intensidad se dibujan sobre la imagen, fuera de MPR
curve-no-roi = Dibuje antes una elipse o un rectángulo
curve-loading = { $count ->
        [one] Leyendo { $count } punto temporal…
       *[other] Leyendo { $count } puntos temporales…
    }
curve-title = Curva tiempo-intensidad
curve-mean = Media
curve-max = Máx.
curve-seconds = s
curve-saved = Curva guardada
curve-none = No hay curva que guardar
curve-failed = Curva no creada: { $reason }
