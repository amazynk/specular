# machine-drafted, needs native review
# Specular — Informe de apertura: por qué una imagen tardó lo que tardó en abrirse (panel de
# información, sección «Rendimiento»; DESIGN.md §12.7).

open-report-title = Rendimiento

## Headlines
open-report-fast = Abierta en { $ms } ms
open-report-display-cache = Mostrada desde la caché de visualización
open-report-slow-read = Almacenamiento lento: el { $percent } del tiempo se fue en leer el archivo ({ $speed } MB/s)
# machine-drafted, needs native review
open-report-slow-read-no-speed = Almacenamiento lento: el { $percent } del tiempo se fue en leer el archivo (archivo pequeño: sobre todo esperando a que responda el almacenamiento)
open-report-archive-read = El { $percent } del tiempo se fue en descomprimir el archivo comprimido
open-report-startup = El { $percent } del tiempo se fue en iniciar { -app-name }
open-report-checked = Decodificada dos veces: el primer resultado del decodificador por hardware se comparó con el de la CPU ({ $percent } del tiempo)
open-report-hardware = Decodificada por el decodificador por hardware, que tardó el { $percent } del tiempo
open-report-decode = Imagen grande: decodificar { $mpx } megapíxeles llevó el { $percent } del tiempo
open-report-upload = El { $percent } del tiempo se fue en enviar la imagen a la GPU
open-report-waiting = El { $percent } del tiempo se fue en esperar a que terminaran otras tareas
open-report-stage = { $stage } llevó el { $percent } del tiempo
open-report-memory-short = Faltaba memoria, así que el sistema pudo tener que usar el intercambio

## Why a decode ran on the processor although a hardware decoder can do it
open-report-sw-not-probed = Decodificada por software: el decodificador por hardware aún se estaba detectando
open-report-sw-no-device = Decodificada por software: { $api } no está disponible en este equipo
open-report-sw-no-codec = Decodificada por software: { $api } no tiene decodificador de { $codec } en esta GPU
open-report-sw-blocked-mismatch = Decodificada por software: el decodificador por hardware se desactivó porque su resultado no coincidía con el de la CPU
open-report-sw-blocked-error = Decodificada por software: el decodificador por hardware se desactivó tras un error
open-report-sw-blocked-lost = Decodificada por software: el decodificador por hardware se desactivó porque dejó de responder
open-report-sw-failed = Decodificada por software: el decodificador por hardware falló con esta imagen
open-report-sw-busy = Decodificada por software: el decodificador por hardware estaba ocupado
open-report-sw-unsupported = Decodificada por software: el decodificador por hardware no admite esta imagen
open-report-sw-unavailable = Decodificada por software: la decodificación por hardware está desactivada hasta que se reinicie { -app-name }
open-report-sw-cpu-only = Decodificada por software: el procesamiento está en «Solo la CPU»
open-report-sw-setting-off = Decodificada por software: la decodificación por hardware está desactivada en la configuración
open-report-sw-unchecked = Decodificada por software: el decodificador por hardware aún no se ha comprobado (se prueba primero con una imagen preparada en segundo plano)
open-report-sw-lower-power = Decodificada por software: con batería, la CPU consume menos para esto
open-report-sw-faster = Decodificada por software: es más rápido en este equipo (unos { $cpu } ms, frente a { $hw } ms con el decodificador por hardware)
open-report-sw-scaled = Decodificada por software: el decodificador por hardware solo decodifica imágenes a tamaño completo

## Hints
open-report-hint-copy-local = Copie el archivo a un disco local para abrirlo más rápido.
open-report-hint-extract = Extraiga el archivo comprimido a una carpeta para abrir sus páginas más rápido.
open-report-hint-try-again = Para volver a probar el decodificador por hardware: Configuración → Rendimiento → Reintentar.
open-report-hint-automatic = Ponga «Dónde se procesan las imágenes» en Automático en Configuración → Rendimiento.
open-report-hint-turn-on = Ponga «Decodificación por hardware» en Automático en Configuración → Rendimiento.
open-report-hint-free-memory = Cierre otros programas para liberar memoria.
open-report-hint-once-only = Solo la primera imagen se comprueba así.

## Rows
open-report-total = Hasta nitidez
open-report-first-pixels = Primeros píxeles
# machine-drafted, needs native review
open-report-whole = Imagen completa visible
open-report-stage-startup = Inicio
open-report-stage-read = Lectura del archivo
open-report-stage-parse = Lectura de la cabecera
open-report-stage-colour = Preparación de la gestión del color
open-report-stage-decode = Decodificación
open-report-stage-upload = Envío a la GPU
open-report-stage-waiting = Espera a otras tareas
open-report-decoder = Decodificada por
open-report-decoder-checked = { $engine }, comprobado con la CPU
open-report-decoder-cache = Caché de visualización
# $scale is the reduction: 4 for a level a quarter of the full width and height.
open-report-decoder-scaled = { $engine }, a 1/{ $scale } del tamaño
open-report-decoder-scaled-full = { $engine }, a tamaño completo y luego reducida a 1/{ $scale }
open-report-decoder-scaled-no-index = { $engine }, a 1/{ $scale } del tamaño, sin el índice de inicio rápido
open-report-colour-conversion = Colores convertidos por

## Advisor: a pattern across recent opens, what would help and by how much (rows)
# $count and $total are counts of opens, $format a file format ("HEIC"), $speed a formatted
# number (MB/s), $low and $high formatted numbers ("2.5").
open-report-advice-recurring = Recurrente
open-report-advice-change = Ayudaría
open-report-advice-gain = Mejora estimada
open-report-advice-software = { $count } de las últimas { $total } imágenes { $format } lentas se { $count ->
    [one] decodificó
   *[other] decodificaron
} por software por este motivo
open-report-advice-slow-storage = { $count } de las últimas { $total } imágenes lentas se { $count ->
    [one] retrasó
   *[other] retrasaron
} al leer el archivo (unos { $speed } MB/s)
# machine-drafted, needs native review
open-report-advice-slow-storage-no-speed = { $count } de las últimas { $total } imágenes lentas se { $count ->
    [one] retrasó
   *[other] retrasaron
} al leer el archivo (archivos pequeños: sobre todo esperando a que responda el almacenamiento)
open-report-advice-archive = { $count } de las últimas { $total } imágenes lentas se { $count ->
    [one] retrasó
   *[other] retrasaron
} al descomprimir el archivo comprimido
open-report-advice-hardware-decoder = Una GPU con decodificador por hardware de { $codec }
open-report-advice-gain-range = Aproximadamente de { $low } a { $high } veces más rápido
open-report-advice-gain-one = Aproximadamente { $low } veces más rápido

## Performance insights: the indicator in the corner and the panel it opens
# machine-drafted, needs native review
# $total is a formatted duration ("240 ms"), $format a file format ("HEIC").
open-report-insights-title = Consejos de rendimiento
open-report-insights-indicator = ¿Por qué va lento?
open-report-insights-announce = Consejo de rendimiento: las imágenes siguen abriéndose despacio por el mismo motivo. Abra «¿Por qué va lento?» para ver qué ayudaría.
open-report-insights-hide = Ocultar
open-report-insights-last = Última imagen lenta: { $total } hasta nitidez
open-report-insights-now = Lo que puede hacer ahora
open-report-insights-hardware = Hardware que ayudaría
open-report-insights-none = Ninguna lentitud recurrente
open-report-insights-none-detail = Los consejos aparecen cuando varias imágenes se abren despacio por el mismo motivo. Para encontrarlos, { -app-name } guarda una semana de aperturas lentas en este equipo; no se envía nada a ningún sitio.
open-report-insights-software = Las imágenes { $format } se decodifican en la CPU
open-report-insights-storage = La lectura de los archivos retrasa las imágenes
open-report-insights-archive = La descompresión de archivos comprimidos retrasa las imágenes
open-report-insights-nothing-now = Ningún ajuste de { -app-name } cambia esto.
open-report-insights-display-cache = Active la caché de visualización: las imágenes que ya ha abierto se abrirán desde una copia rápida en este equipo.
open-report-insights-prefetch = Mantenga más imágenes vecinas listas, para que las siguientes se preparen mientras mira.
open-report-insights-hw-ssd = Una unidad interna rápida (un SSD NVMe) para estos archivos
open-report-insights-action-turn-on = Activar
open-report-insights-action-try-again = Reintentar
open-report-insights-action-automatic = Poner en Automático
open-report-insights-dismiss-storage = No volver a mostrar para el almacenamiento
open-report-insights-dismiss-decoder = No volver a mostrar para la decodificación
open-report-insights-off = Desactivar los consejos
open-report-insights-on = Activar los consejos
open-report-insights-restore = Volver a mostrar los consejos ocultos
open-report-insights-power = Este equipo está ahorrando energía (con batería o en un modo de ahorro de energía), lo que ralentiza la decodificación en la CPU. Conectado a la corriente, las imágenes pueden abrirse más rápido.

## Performance insights in the gallery: the same, about making thumbnails
# machine-drafted, needs native review
# $count and $total are counts of thumbnails, $total in -last-thumbnail a formatted duration
# ("240 ms"), $format a file format ("HEIC"), $speed a formatted number (MB/s).
open-report-insights-announce-thumbnails = Consejo de rendimiento: las miniaturas siguen apareciendo despacio por el mismo motivo. Abra «¿Por qué va lento?» para ver qué ayudaría.
open-report-insights-last-thumbnail = Última miniatura lenta: { $total } en crearse
open-report-insights-thumbnails-software = Las miniaturas de imágenes { $format } se decodifican en la CPU
open-report-insights-thumbnails-storage = La lectura de los archivos retrasa las miniaturas
open-report-insights-thumbnails-archive = La descompresión de archivos comprimidos retrasa las miniaturas
open-report-insights-thumbnail-cache = Las miniaturas no se pueden guardar en este equipo (la caché de miniaturas no está disponible), así que se vuelven a crear cada vez que se muestra la carpeta.
open-report-advice-thumbnails-software = { $count } de las últimas { $total } miniaturas { $format } lentas se { $count ->
    [one] decodificó
   *[other] decodificaron
} por software por este motivo
open-report-advice-thumbnails-slow-storage = { $count } de las últimas { $total } miniaturas lentas se { $count ->
    [one] retrasó
   *[other] retrasaron
} al leer el archivo (unos { $speed } MB/s)
# machine-drafted, needs native review
open-report-advice-thumbnails-slow-storage-no-speed = { $count } de las últimas { $total } miniaturas lentas se { $count ->
    [one] retrasó
   *[other] retrasaron
} al leer el archivo (archivos pequeños: sobre todo esperando a que responda el almacenamiento)
open-report-advice-thumbnails-archive = { $count } de las últimas { $total } miniaturas lentas se { $count ->
    [one] retrasó
   *[other] retrasaron
} al descomprimir el archivo comprimido
