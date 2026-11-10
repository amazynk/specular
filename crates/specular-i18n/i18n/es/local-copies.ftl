# machine-drafted, needs native review
# Local copies for offline editing (the Library).

settings-library-local-copies-limit = Límite de copias locales
settings-library-local-copies-limit-help = El espacio máximo que pueden ocupar las copias locales en la carpeta de datos de Specular.

local-keep-menu = Guardar una copia local…
local-remove-menu = Quitar la copia local…
local-keep-location-menu = Guardar una copia local de esta ubicación…
local-remove-location-menu = Quitar la copia local de esta ubicación…
local-keep-album-menu = Guardar una copia local de este álbum…
local-remove-album-menu = Quitar la copia local de este álbum…
local-remove-held-elsewhere = Estas copias pertenecen a una ubicación o un álbum guardado. Quítalas allí.

local-keep-ask = ¿Guardar una copia local de { $item }?
local-keep-detail = { $count ->
    [one] 1 foto.
    [many] { $count } de fotos.
   *[other] { $count } fotos.
}
    Originales: los archivos tal como están.
    Proxies: los raw a 2560 píxeles, suficiente para revelar. Los demás archivos se copian enteros.
    Las copias van a la carpeta de datos de Specular. Los originales no cambian.
local-keep-originals = Guardar originales
local-keep-proxies = Guardar proxies
local-keeping = Copiando { $item }…
local-kept = { $item }: { $count ->
    [one] 1 copia local
    [many] { $count } de copias locales
   *[other] { $count } copias locales
}, { $size }.
local-kept-full = { $count ->
    [one] 1 foto no cupo en el límite de tamaño.
    [many] { $count } de fotos no cupieron en el límite de tamaño.
   *[other] { $count } fotos no cupieron en el límite de tamaño.
}
local-kept-unreachable = { $count ->
    [one] No se pudo leer 1 foto.
    [many] No se pudieron leer { $count } de fotos.
   *[other] No se pudieron leer { $count } fotos.
}

local-remove-ask = ¿Quitar la copia local de { $item }?
local-remove-detail = Se borran las copias de la carpeta de datos de Specular. Los originales no se tocan. Los cambios pendientes se conservan.
local-remove-ok = Quitar copia local
local-removed = Copia local de { $item } quitada, { $size } liberados.

local-banner-offline = { $location } está sin conexión. Trabajas con copias locales.
local-banner-offline-many = { $count } ubicaciones están sin conexión. Trabajas con copias locales.
local-banner-waiting = { $count ->
    [one] 1 cambio espera al original.
    [many] { $count } de cambios esperan a los originales.
   *[other] { $count } cambios esperan a los originales.
}
local-banner-conflicts = { $count ->
    [one] 1 archivo cambió mientras estabas sin conexión.
    [many] { $count } de archivos cambiaron mientras estabas sin conexión.
   *[other] { $count } archivos cambiaron mientras estabas sin conexión.
}
local-review = Revisar…
local-sidebar-originals = copia local
local-sidebar-proxies = proxies locales

local-synced = { $count ->
    [one] 1 cambio escrito en su original.
    [many] { $count } de cambios escritos en sus originales.
   *[other] { $count } cambios escritos en sus originales.
}
local-sync-failed = { $count ->
    [one] No se pudo escribir 1 cambio. Sigue en espera.
    [many] No se pudieron escribir { $count } de cambios. Siguen en espera.
   *[other] No se pudieron escribir { $count } cambios. Siguen en espera.
}

local-conflict-title = { $file } cambió mientras estabas sin conexión. ¿Qué versión guardas: { $what }?
local-conflict-yours = La tuya: { $text }
local-conflict-theirs = En el disco: { $text }
local-conflict-later = Cancelar decide más tarde. No se sobrescribe nada.
local-keep-mine = Guardar la tuya
local-keep-theirs = Guardar la del archivo
local-what-marks = valoración
local-what-keywords = palabras clave
local-what-fields = campos IPTC
local-what-edit = edición
local-stars = { $count ->
    [0] Sin estrellas
    [one] 1 estrella
    [many] { $count } de estrellas
   *[other] { $count } estrellas
}
local-label = { $label ->
    [1] Rojo
    [2] Amarillo
    [3] Verde
   *[4] Azul
}
local-pick = Seleccionada
local-reject = Rechazada
local-no-keywords = sin palabras clave
local-changed-elsewhere = cambiado en otro sitio
local-photo-changed = la propia foto cambió
local-edit-here = editada aquí, { $when }
local-edit-elsewhere = editada en otro sitio, { $when }
local-edit-none = sin edición

local-export-ask = { $count ->
    [one] 1 foto está sin conexión. ¿Exportarla desde su proxy?
    [many] { $count } de fotos están sin conexión. ¿Exportarlas desde sus proxies?
   *[other] { $count } fotos están sin conexión. ¿Exportarlas desde sus proxies?
}
local-export-detail = Los proxies miden 2560 píxeles en el lado largo. Los originales dan el tamaño completo.
local-export-proxy = Exportar desde proxies
local-export-wait = Esperar a los originales
local-export-waiting = La exportación empieza cuando vuelvan los originales.

local-panel-title = Copias locales
local-panel-used = En uso { $used } de { $limit }.
local-panel-originals = { $count ->
    [one] 1 original
    [many] { $count } de originales
   *[other] { $count } originales
}
local-panel-proxies = { $count ->
    [one] 1 foto, raw como proxies
    [many] { $count } de fotos, raw como proxies
   *[other] { $count } fotos, raw como proxies
}
local-panel-remove = Quitar…
local-panel-footer = Guardadas en { $folder }. Los originales nunca cambian.
local-panel-keywords = sin conexión copias locales proxies espacio
