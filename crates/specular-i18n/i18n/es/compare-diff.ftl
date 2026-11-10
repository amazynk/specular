# Compare: split, signed difference, flicker, alignment, metrics and the pixel probe
# (DESIGN.md §6.3).

## Actions (palette, cheat sheet, remote control)
palette-compare-with = Comparar con…
palette-compare-with-help = Comparar esta imagen con otra que elija.
palette-compare-side = Comparar en paralelo
palette-compare-side-help = Mostrar las imágenes comparadas una junto a otra.
palette-compare-split = Comparar con división
palette-compare-split-help = Mostrar la segunda imagen sobre la primera, cortada por un divisor que puede arrastrar.
palette-compare-split-direction = Girar el divisor
palette-compare-split-direction-help = Poner el divisor en horizontal en vez de vertical, o al revés.
palette-compare-overlay = Comparar en superposición
palette-compare-overlay-help = Mostrar la segunda imagen sobre la primera, fundida.
palette-compare-difference = Comparar la diferencia
palette-compare-difference-help = Mostrar dónde difieren las imágenes; otra vez para el siguiente tipo.
palette-compare-flicker = Alternar entre las imágenes comparadas
palette-compare-flicker-help = Mostrar la otra imagen al instante.
palette-compare-auto-flicker = Parpadeo automático
palette-compare-auto-flicker-help = Cambiar solo entre las dos imágenes, o detenerse.
palette-compare-align = Alinear las imágenes comparadas
palette-compare-align-help = Alinear imágenes de distinto tamaño: arriba a la izquierda, centradas, escaladas o de forma automática.
palette-compare-metric-space = Cambiar el espacio de medida
palette-compare-metric-space-help = Medir en luz lineal o en sRGB codificado.
palette-compare-whole-image = Medir toda la imagen
palette-compare-whole-image-help = Medir toda la imagen en vez de lo visible, o al revés.
palette-compare-probe = Sonda de píxeles de la comparación
palette-compare-probe-help = Mostrar los valores de ambas imágenes bajo el puntero.
palette-compare-link = Vincular o desvincular las imágenes comparadas
palette-compare-link-help = Ampliar y desplazar cada una de las dos imágenes por separado, o juntas de nuevo.
palette-compare-close = Cerrar la comparación
palette-compare-close-help = Volver a la galería.
disabled-compare-closed = Abra primero la comparación
disabled-compare-not-two = Funciona con dos imágenes

## Cheat sheet
cheatsheet-compare-split = División o en paralelo
cheatsheet-compare-split-direction = Divisor horizontal o vertical
cheatsheet-compare-auto-flicker = Parpadeo automático
cheatsheet-compare-align = Siguiente alineación
cheatsheet-compare-space = Espacio de medida
cheatsheet-compare-whole = Toda la imagen o lo visible
cheatsheet-compare-probe = Sonda de píxeles
cheatsheet-compare-link = Vincular o desvincular dos imágenes

## Settings
settings-gallery-compare-metric-space = Espacio de medida de la comparación
settings-gallery-compare-metric-space-help = Dónde mide la comparación MAE, MSE, PSNR y SSIM. La luz lineal es lo que muestra la pantalla. sRGB coincide con otras herramientas en archivos de 8-bit.
settings-gallery-compare-metric-space-linear = Luz lineal
settings-gallery-compare-metric-space-srgb = sRGB codificado
settings-gallery-compare-flicker-limit = Parpadeo automático más rápido
settings-gallery-compare-flicker-limit-help = Cambios de imagen por segundo como máximo. Por encima de 3, el parpadeo puede provocar crisis epilépticas.

## Toolbar and captions
compare-split = División
compare-split-down = Divisor vertical
compare-split-across = Divisor horizontal
compare-link = Vinculadas
compare-auto-flicker = Auto
compare-flicker-rate = { $rate } cambios por segundo
compare-gain = Ganancia ×{ $gain }
compare-probe = Sonda
compare-kind-signed = Error con signo
compare-kind-on-image = Error sobre la imagen
compare-align-top-left = Arriba a la izquierda
compare-align-centre = Centrado
compare-align-scaled = B escalada a A
compare-align-auto = Alineación automática
compare-space-linear = Luz lineal
compare-space-srgb = sRGB
compare-space-help-linear = Medido en luz lineal, tal como se muestra tras el mapeo de tonos. Aquí el PSNR no es el PSNR habitual de sRGB de 8-bit.
compare-space-help-srgb = Medido sobre valores sRGB codificados, como hacen otras herramientas con archivos de 8-bit. El HDR por encima del blanco SDR cuenta como blanco.
compare-space-help-common = ΔE2000 es siempre CIELAB (D65), con el blanco SDR como L* 100 y el HDR recortado. Las imágenes de más de { $size } píxeles se miden en una copia de { $size } píxeles.
compare-region-view = Lo visible
compare-region-whole = Toda la imagen
compare-ssim = SSIM { $ssim }
compare-ssim-small = SSIM necesita 11 píxeles
compare-delta-e = ΔE { $mean } ({ PERCENT(95) }: { $p95 }, máx. { $max })
compare-delta-e-hdr = HDR recortado para ΔE
compare-aligned-centre = Centrado, { $width } × { $height } comparados
compare-aligned-scaled = B escalada { $scale }×, { $width } × { $height } comparados
compare-aligned-auto = Alineado { $angle }° { $scale }×, { $width } × { $height } comparados
compare-aligning = Alineando…
compare-align-failed = No se pudieron alinear estas imágenes. Se vuelve a arriba a la izquierda.
compare-split-label = { $b } sobre { $a }, divisor al { PERCENT($percent) }
compare-showing-which = Mostrando { $which }: { $name }
compare-auto-flicker-on = Parpadeando, { $rate } cambios por segundo
compare-auto-flicker-off = Parpadeo detenido
compare-linked = Imágenes vinculadas
compare-unlinked = Imágenes desvinculadas: cada una se desplaza por separado
compare-with-title = Comparar con
compare-probe-delta-e = ΔE2000
compare-probe-a11y = A: { $a }. B: { $b }. ΔE2000 { $delta }.
compare-a11y-divider = Divisor
compare-a11y-space = Espacio de medida
compare-a11y-align = Alineación
