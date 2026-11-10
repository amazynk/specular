# machine-drafted, needs native review
## Astronomy interop: DS9 regions, plate solving, SAMP, HiPS sky surveys.

palette-science-load-regions = Superponer regiones DS9…
palette-science-load-regions-help = Dibujar en la imagen las formas de un archivo de regiones DS9 (.reg).
palette-science-export-regions = Guardar regiones como DS9…
palette-science-export-regions-help = Guardar las mediciones y las regiones superpuestas como archivo de regiones DS9.
palette-science-edit-regions = Editar regiones como mediciones
palette-science-edit-regions-help = Convertir las regiones superpuestas en mediciones que se pueden mover y cambiar.
palette-science-clear-regions = Quitar las regiones superpuestas
palette-science-clear-regions-help = Quitar las regiones DS9 de la imagen.
palette-science-plate-solve = Resolución astrométrica
palette-science-plate-solve-help = Hallar las coordenadas celestes de la imagen con ASTAP o astrometry.net. Volver a ejecutar para detener.
palette-science-save-plate-solution = Guardar la solución astrométrica
palette-science-save-plate-solution-help = Guardar las coordenadas celestes halladas junto a la imagen, como archivo .wcs.
palette-science-sky-survey = Sondeo del cielo de fondo
palette-science-sky-survey-help = Mostrar el siguiente sondeo del cielo alrededor de la imagen, o ninguno.
palette-science-samp-send-image = Enviar imagen a aplicaciones SAMP
palette-science-samp-send-image-help = Abrir esta imagen en DS9, Aladin y las demás aplicaciones del hub SAMP.
palette-science-samp-send-position = Enviar posición a aplicaciones SAMP
palette-science-samp-send-position-help = Apuntar las demás aplicaciones a la posición celeste bajo el puntero.

disabled-samp-off = Conectar primero a un hub SAMP (Configuración › Integración).

settings-integration-samp = Conectar a un hub SAMP
settings-integration-samp-help = Intercambiar imágenes, tablas y posiciones celestes con DS9, TOPCAT, Aladin y otras aplicaciones de astronomía. Solo se usa un hub en este equipo.
settings-integration-astap-path = Programa ASTAP
settings-integration-astap-path-help = El resolvedor astrométrico ASTAP, si no se encuentra solo.
settings-integration-solve-field-path = Programa astrometry.net
settings-integration-solve-field-path-help = El programa solve-field de astrometry.net, si no se encuentra solo.

astro-regions-filter = Regiones DS9
astro-regions-loaded = { $name }: { $count ->
        [one] { $count } región.
       *[other] { $count } regiones.
    }
astro-regions-refused = { $count ->
        [one] { $count } línea sin leer.
       *[other] { $count } líneas sin leer.
    } Primera: línea { $line }, { $reason }.
astro-regions-approximate = { $count ->
        [one] { $count } panda dibujado por sus bordes interior y exterior.
       *[other] { $count } pandas dibujados por sus bordes interior y exterior.
    }
astro-regions-none-here = { $name }: ninguna región cae en esta imagen.
astro-regions-failed = No se pudo leer { $name }: { $reason }
astro-regions-cleared = Regiones quitadas.
astro-regions-nothing = No hay regiones ni mediciones que guardar.
astro-regions-saved = { $name } guardado.
astro-regions-skipped = { $count ->
        [one] Se omitió { $count } forma fuera de las coordenadas celestes.
       *[other] Se omitieron { $count } formas fuera de las coordenadas celestes.
    }
astro-regions-save-failed = No se pudieron guardar las regiones: { $reason }
astro-regions-edited = { $count ->
        [one] { $count } región es ahora una medición.
       *[other] { $count } regiones son ahora mediciones.
    }
astro-regions-no-wcs = Las regiones celestes necesitan una imagen con coordenadas celestes.

astro-solve-no-solver = No se encontró ningún resolvedor astrométrico. Instalar ASTAP o astrometry.net, o elegir uno en Configuración › Integración.
astro-solve-running = Resolviendo con { $solver }… { $seconds } s
astro-solve-stop = Detener
astro-solve-stopped = Resolución astrométrica detenida.
astro-solve-done = Resuelto: { $ra }, { $dec }, { $scale } por píxel.
astro-solve-failed = Falló la resolución astrométrica: { $reason }
astro-solve-not-local = Solo se pueden resolver imágenes de una carpeta de este equipo.
astro-solve-none = Esta imagen aún no tiene solución astrométrica.
astro-solve-saved = { $name } guardado. La imagen en sí no cambia.
astro-solve-sidecar = Coordenadas celestes de { $name }.

astro-samp-connected = Conectado al hub SAMP.
astro-samp-no-hub = SAMP: { $reason }. Se reintentará cuando se inicie un hub.
astro-samp-lost = El hub SAMP se detuvo. Se reintentará cuando se inicie uno.
astro-samp-sent = { $count ->
        [one] Enviado a { $count } aplicación.
       *[other] Enviado a { $count } aplicaciones.
    }
astro-samp-nobody = Ninguna aplicación del hub escucha esto.
astro-samp-failed = SAMP: { $reason }
astro-samp-refused = SAMP: { $url } no está en este equipo, así que no se abre.
astro-samp-pointed = Apuntado a { $ra }, { $dec }.
astro-samp-off-image = { $ra }, { $dec } no está en esta imagen.
astro-samp-no-table = SAMP: esa tabla no está superpuesta aquí.
astro-samp-rows = { $count ->
        [one] { $count } fuente del catálogo resaltada.
       *[other] { $count } fuentes del catálogo resaltadas.
    }

astro-survey-off = Sondeo del cielo desactivado.
astro-survey-on = Sondeo del cielo: { $name }. { $credit }
astro-survey-failed = No se puede mostrar { $name }: { $reason }
astro-survey-credit = { $name }: { $credit }

net-what-sky-survey = Teselas de sondeo del cielo de { $host }
