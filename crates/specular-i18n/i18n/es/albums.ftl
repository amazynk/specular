# machine-drafted, needs native review
# Specular — Library albums, smart albums and the date timeline.

## Sidebar
albums-title = Álbumes
albums-new = Nuevo álbum…
albums-new-smart = Nuevo álbum inteligente…
albums-save-search = Guardar la búsqueda como álbum inteligente…
albums-smart = Álbum inteligente
# The album the photos go to when added without choosing.
albums-target = Lo añadido va aquí
albums-group-count = { $count ->
    [one] { $count } álbum
   *[other] { $count } álbumes
}
albums-need-library = Los álbumes están en la Biblioteca. Añade primero esta carpeta a la Biblioteca.

## Screen readers: the buttons of album and group rows
albums-a11y-rename = Renombrar { $album }
albums-a11y-edit = Editar { $album }
albums-a11y-delete = Eliminar el álbum { $album }
albums-a11y-rename-group = Renombrar el grupo { $group }
albums-a11y-delete-group = Eliminar el grupo { $group }

## Sheets
albums-name = Nombre
albums-unnamed = Aún sin nombre
albums-new-title = Nuevo álbum
albums-new-smart-title = Nuevo álbum inteligente
albums-edit-smart-title = Editar álbum inteligente
albums-rename-title = Renombrar álbum
albums-rename-group-title = Renombrar grupo
albums-create = Crear
albums-rename = Renombrar
albums-save = Guardar
albums-groups-note = Usa / para ponerlo en un grupo: Cliente X / selección.
albums-name-empty = Escribe un nombre.
albums-name-taken = { $album } ya existe.
albums-name-clash = { $album } es un álbum, así que no puede ser también un grupo.

## Smart album rules
albums-rules = Reglas
albums-rules-note = Las reglas se aplican de arriba abajo. Las reglas vacías se omiten.
albums-rules-none = Añade al menos una regla.
albums-rule-label = Regla { $number }
albums-rule-empty = Regla vacía
albums-rule-add = Añadir una regla
albums-rule-combine = Combinar con las reglas de arriba
albums-rule-first = Fotos que cumplen
albums-rule-and = Y
albums-rule-or = O
albums-rule-except = Excepto

## Adding and removing photos
albums-add-menu = Añadir a un álbum…
albums-remove-menu = Quitar del álbum
albums-add-title = { $count ->
    [one] Añadir { $count } foto a un álbum
   *[other] Añadir { $count } fotos a un álbum
}
albums-add = Añadir
albums-add-new = Nuevo álbum
albums-add-list = Álbumes
albums-add-none = Aún no hay álbumes. Escribe un nombre para crear uno.
albums-add-note = O escribe un nombre para un álbum nuevo.
albums-added = { $count ->
    [one] Se añadió { $count } foto a { $album }.
   *[other] Se añadieron { $count } fotos a { $album }.
}
albums-added-missing = { $count ->
    [one] { $count } no está en la Biblioteca, así que no se añadió.
   *[other] { $count } no están en la Biblioteca, así que no se añadieron.
}
albums-created = Se creó { $album }.
albums-removed = { $count ->
    [one] Se quitó { $count } foto de { $album }.
   *[other] Se quitaron { $count } fotos de { $album }.
}
albums-remove = Quitar
albums-remove-confirm = { $count ->
    [one] ¿Quitar { $count } foto de { $album }?
   *[other] ¿Quitar { $count } fotos de { $album }?
}
albums-remove-confirm-detail = Los archivos se quedan en el disco y en la Biblioteca.
albums-delete = Eliminar
albums-delete-confirm = ¿Eliminar el álbum { $album }?
albums-delete-group-confirm = { $count ->
    [one] ¿Eliminar el grupo { $group } y su { $count } álbum?
   *[other] ¿Eliminar el grupo { $group } y sus { $count } álbumes?
}
albums-delete-confirm-detail = Las fotos se quedan en el disco y en la Biblioteca.
albums-deleted = { $count ->
    [one] Álbum eliminado.
   *[other] { $count } álbumes eliminados.
}
albums-failed = No se pudieron cambiar los álbumes: { $reason }

## Date timeline
albums-timeline = Cronología
albums-timeline-description = Elige un periodo para mostrar solo sus fotos. Shift+clic elige varios seguidos.
albums-timeline-picked = Mostrando { $period }
albums-timeline-cleared = Mostrando todas las fechas
