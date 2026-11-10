# machine-drafted, needs native review
# Specular — Library (DESIGN §10).

palette-library = Biblioteca

## Top bar
library-title = Biblioteca
library-locations-button = Ubicaciones

## Locations sidebar
library-locations = Ubicaciones
library-all = Todas las ubicaciones
library-add = Añadir ubicación…
library-remove = Quitar { $location } de la biblioteca
library-offline = Sin conexión
library-location-pending = { $count } por indexar
library-a11y-location = { $location }, { $count ->
    [one] { $count } imagen
   *[other] { $count } imágenes
}
library-a11y-location-offline = { $location }, Sin conexión, { $count ->
    [one] { $count } imagen
   *[other] { $count } imágenes
}

## Indexing progress
library-walking = Imágenes encontradas: { $count } · { $location }
# machine-drafted, needs native review
library-walking-count = Imágenes encontradas: { $count }
library-reading = Indexando… { $done } de { $total }
library-held = La indexación se pausa mientras navegas
library-up-to-date = Al día
# machine-drafted, needs native review
library-stopped = Indexación detenida
library-stopped-pending = Indexación detenida · { $count } por indexar
library-resume = Reanudar
# machine-drafted, needs native review
# The indexer stopped by itself (a database error, a bug) or stopped moving; $reason says why.
library-halted = Indexación detenida: { $reason }
library-reason-stalled = no se ha movido nada en { $minutes ->
    [one] un minuto
   *[other] { $minutes } minutos
}. Puede que un archivo o una unidad no responda.
library-retry = Reintentar
library-retry-description = Vuelve a iniciar la indexación. Lo indexado se puede seguir buscando.
library-save-diagnostics = Guardar paquete de diagnóstico…
library-stop-description = Detiene la indexación. Lo indexado hasta ahora se puede seguir buscando; Reanudar o Volver a analizar (F5) continúa.
library-resume-description = Recorre de nuevo todas las ubicaciones e indexa lo que falta.
library-opening = Abriendo la biblioteca…
library-searching = Buscando…

## Empty and error states
library-empty = Aún no hay ubicaciones. Añade una carpeta: sus imágenes y las de sus subcarpetas aparecen aquí en una sola cuadrícula.
library-empty-offline = Estas ubicaciones están sin conexión. Sus imágenes volverán a verse cuando vuelvan.
library-indexing = Buscando imágenes…
# machine-drafted, needs native review
library-remove-confirm = ¿Quitar «{ $location }» de la biblioteca?
library-remove-confirm-detail = Tus fotos se quedan en el disco, con sus valoraciones, etiquetas y palabras clave. La biblioteca olvida lo que indexó allí; si vuelves a añadir la carpeta, se indexa de nuevo.
library-remove-confirm-ok = Quitar
library-indexing-in = Buscando imágenes en { $location }… { $count } encontradas hasta ahora
# machine-drafted, needs native review
library-empty-stopped = La indexación se detuvo antes de leer estas imágenes. Reanudar continúa desde ahí.
library-unavailable = No se pudo abrir la biblioteca: { $reason }
library-read-only = Esta biblioteca la escribió un Specular más reciente: se puede buscar en ella, pero no actualizar.

## Screen readers
library-a11y-list = Biblioteca, { $count ->
    [one] { $count } imagen
   *[other] { $count } imágenes
}
library-a11y-added = { $location } añadida a la biblioteca
library-a11y-removed = { $location } quitada de la biblioteca

## Adding the home folder
# machine-drafted, needs native review
library-home-confirm = ¿Añadir todo «{ $folder }»?
library-home-confirm-detail = Contiene archivos de programas, descargas y cachés, no solo fotos. La indexación tarda más y la biblioteca se llena de iconos y otras imágenes.
library-home-confirm-pictures = Su carpeta Imágenes contiene solo sus imágenes.
library-home-confirm-use-pictures = Añadir Imágenes
library-home-confirm-keep = Añadir carpeta personal

## Folder watches
# machine-drafted, needs native review
library-unwatched = Algunas carpetas no se vigilan
library-unwatched-detail = { $minutes ->
    [one] Los cambios aparecen en un minuto
   *[other] Los cambios aparecen en { $minutes } min
}
library-unwatched-description = Specular se quedó sin vigilancias de carpetas. Abre la ayuda sobre cómo subir el límite.

## Settings → Library
# machine-drafted, needs native review
settings-section-library = Biblioteca
settings-library-skip-folders = Omitir carpetas llamadas
settings-library-skip-folders-help = La biblioteca no busca en carpetas con estos nombres, separados por comas. Las carpetas ocultas siempre se omiten.
settings-library-skip-marked = Omitir carpetas que contengan
settings-library-skip-marked-help = Tampoco en carpetas que contengan alguno de estos: .git para repositorios de código, pyvenv.cfg para entornos de Python, CACHEDIR.TAG para cachés. La carpeta de una ubicación nunca se omite.
settings-library-show-timeline = Línea de tiempo
settings-library-show-timeline-help = Muestra cuántas fotos tiene cada año, mes o día, encima de la cuadrícula de la biblioteca.
