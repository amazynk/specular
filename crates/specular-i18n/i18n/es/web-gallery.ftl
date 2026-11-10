# Specular — Galerías para clientes: una carpeta HTML estática de fotos, y leer de vuelta la selección del cliente.

# machine-drafted, needs native review

palette-client-gallery = Galería para cliente…
palette-client-gallery-help = Guardar las fotos como una carpeta de página web para un cliente, con una selección que puede devolver.
palette-import-client-picks = Importar selección del cliente…
palette-import-client-picks-help = Marcar las fotos que un cliente eligió en su galería.

webgallery-heading = Galería para cliente
webgallery-subtitle = { $count ->
    [one] Una foto
   *[other] { $count } fotos
}
webgallery-title-label = Título
webgallery-title-default = Pruebas
webgallery-watermark-label = Texto de marca de agua (vacío: ninguna)
webgallery-source = Fotos
webgallery-source-current = Esta foto
webgallery-source-selection = Selección
webgallery-source-album = Álbum «{ $name }»
webgallery-source-search = Resultados de búsqueda
webgallery-source-folder = Esta carpeta
webgallery-source-picks = Elegidas
webgallery-source-value = { $what } ({ $count })
webgallery-theme = Tema
webgallery-theme-light = Claro
webgallery-theme-dark = Oscuro
webgallery-theme-minimal = Mínimo
webgallery-size = Lado largo
webgallery-size-value = { $px } px
webgallery-format = Formato
webgallery-captions = Bajo cada foto
webgallery-captions-off = Nada
webgallery-captions-name = Nombre de archivo
webgallery-captions-title = Título
webgallery-captions-all = Nombre de archivo, título y pie de foto
webgallery-exif = Mostrar cámara y exposición
webgallery-location = Conservar la ubicación (GPS) en las fotos
webgallery-note-private = Una galería estática no se puede proteger con contraseña.
webgallery-note-share = Cualquiera que tenga la carpeta o su dirección ve todas las fotos. Comparta el enlace solo con su cliente, o use el control de acceso de su alojamiento web.
webgallery-note-picks = Los clientes eligen con un corazón. La selección queda en su navegador hasta que le envían la lista.
webgallery-write = Crear galería…
webgallery-busy = Escribiendo { $done } de { $total }…
webgallery-stop = Detener
webgallery-done = { $count ->
    [one] Galería creada: una foto en { $folder }
   *[other] Galería creada: { $count } fotos en { $folder }
}
webgallery-done-failed = { $failed ->
    [one] No se pudo escribir una foto.
   *[other] No se pudieron escribir { $failed } fotos.
}
webgallery-stopped = Galería detenida. No se conservó nada.
webgallery-failed = Galería no creada: { $reason }
webgallery-nothing = No se pudo escribir ninguna foto.

webgallery-page-count = { $count ->
    [one] Una foto
   *[other] { $count } fotos
}
webgallery-page-hint = Toque el corazón en las fotos que quiera. Su selección queda en este navegador hasta que la envíe.
webgallery-page-picked = Elegidas: {"{"}n{"}"}
webgallery-page-pick = Elegir
webgallery-page-copy = Copiar mi selección
webgallery-page-copied = Copiada. Pegue la lista en un mensaje.
webgallery-page-copy-by-hand = Copie la lista seleccionada y péguela en un mensaje.
webgallery-page-download-txt = Descargar picks.txt
webgallery-page-download-json = Descargar picks.json
webgallery-page-clear = Borrar selección
webgallery-page-clear-ask = ¿Borrar toda su selección en este navegador?
webgallery-page-no-storage = Este navegador no guardará su selección. Envíela antes de cerrar la página.
webgallery-page-noscript = Para elegir hace falta JavaScript. Las fotos se abren igualmente.
webgallery-page-previous = Anterior
webgallery-page-next = Siguiente
webgallery-page-close = Cerrar
webgallery-page-position = {"{"}i{"}"} de {"{"}n{"}"}
webgallery-page-viewer = Visor de fotos
webgallery-page-picks = Su selección

clientpicks-heading = Importar selección del cliente
clientpicks-subtitle = { $count ->
    [one] Comparada con una foto mostrada
   *[other] Comparada con las { $count } fotos mostradas
}
clientpicks-paste-label = Nombres (pegue la lista del cliente)
clientpicks-choose = Elegir archivo de selección…
clientpicks-from-file = Desde { $name }
clientpicks-apply-as = Marcarlas
clientpicks-as-pick = Como elegidas
clientpicks-as-stars = { $stars ->
    [one] Una estrella
   *[other] { $stars } estrellas
}
clientpicks-as-label = Etiqueta { $label }
clientpicks-empty = Pegue la lista que envió el cliente, o elija su picks.txt o picks.json.
clientpicks-matched = { $matched } de { $total } nombres coinciden con fotos de aquí.
clientpicks-missing = No encontrados: { $names }
clientpicks-ambiguous = En más de una carpeta, sin cambios: { $names }
clientpicks-more = { $names } y { $more } más
clientpicks-apply = Aplicar
clientpicks-applied = { $count ->
    [one] Una foto marcada.
   *[other] { $count } fotos marcadas.
}
clientpicks-unchanged = Esas fotos ya estaban marcadas.
clientpicks-read-failed = No se puede leer { $name }: { $reason }
