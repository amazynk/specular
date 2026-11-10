# The play queue: what plays next, repeat and shuffle, playlists (M3U, PLS, XSPF), the
# queue panel and the sound transport's buttons.

# machine-drafted, needs native review
cheatsheet-video-queue = Cola, repetir y aleatorio
# machine-drafted, needs native review
palette-next-track = Elemento siguiente
# machine-drafted, needs native review
palette-next-track-help = Reproducir el siguiente elemento de la cola o, si no, el siguiente archivo del mismo tipo de la carpeta.
# machine-drafted, needs native review
palette-prev-track = Elemento anterior
# machine-drafted, needs native review
palette-prev-track-help = Reproducir el elemento anterior. Pasados 3 segundos, empezar este de nuevo.
# machine-drafted, needs native review
palette-cycle-repeat = Repetir
# machine-drafted, needs native review
palette-cycle-repeat-help = Repetir desactivado, todo o uno.
# machine-drafted, needs native review
palette-shuffle = Aleatorio
# machine-drafted, needs native review
palette-shuffle-help = Reproducir en orden aleatorio, cada elemento una vez por ronda.
# machine-drafted, needs native review
palette-play-queue = Cola de reproducción
# machine-drafted, needs native review
palette-play-queue-help = Ver lo que sonará después: reordenar, quitar, guardar.
# machine-drafted, needs native review
palette-queue-play-next = Reproducir a continuación
# machine-drafted, needs native review
palette-queue-play-next-help = Reproducir el archivo (en la galería, la selección) tras el elemento actual.
# machine-drafted, needs native review
palette-queue-add = Añadir a la cola
# machine-drafted, needs native review
palette-queue-add-help = Añadir el archivo (en la galería, la selección) al final de la cola.
# machine-drafted, needs native review
palette-save-queue = Guardar la cola como lista…
# machine-drafted, needs native review
palette-save-queue-help = Guardar la cola como lista de reproducción M3U8 o XSPF.
# machine-drafted, needs native review
transport-prev-track = Elemento anterior
# machine-drafted, needs native review
transport-next-track = Elemento siguiente
# machine-drafted, needs native review
queue-title = Cola de reproducción
# machine-drafted, needs native review
queue-summary = { $count ->
    [one] 1 elemento
   *[other] { $count } elementos
}
# machine-drafted, needs native review
queue-summary-time = { $count ->
    [one] 1 elemento
   *[other] { $count } elementos
} · { $time }
# machine-drafted, needs native review
queue-empty = La cola está vacía.
    Añada archivos de sonido y vídeos con Reproducir a continuación o Añadir a la cola, o abra una lista de reproducción.
# machine-drafted, needs native review
queue-empty-short = La cola está vacía.
# machine-drafted, needs native review
queue-playing = Sonando
# machine-drafted, needs native review
queue-repeat = Repetir
# machine-drafted, needs native review
queue-repeat-off = Repetir: no
# machine-drafted, needs native review
queue-repeat-all = Repetir: todo
# machine-drafted, needs native review
queue-repeat-one = Repetir: uno
# machine-drafted, needs native review
queue-shuffle = Aleatorio
# machine-drafted, needs native review
queue-shuffle-on = Aleatorio: sí
# machine-drafted, needs native review
queue-shuffle-off = Aleatorio: no
# machine-drafted, needs native review
queue-save = Guardar como lista…
# machine-drafted, needs native review
queue-save-tip = Guardar la cola como lista de reproducción M3U8 o XSPF
# machine-drafted, needs native review
queue-clear = Vaciar…
# machine-drafted, needs native review
queue-clear-tip = Vaciar la cola. Los archivos siguen en el disco.
# machine-drafted, needs native review
queue-remove = Quitar de la cola (el archivo sigue en el disco)
# machine-drafted, needs native review
queue-network-item = Elemento de red, no se reproduce: { $address }
# machine-drafted, needs native review
queue-artist-title = { $artist } – { $title }
# machine-drafted, needs native review
queue-at-end = No hay elemento siguiente.
# machine-drafted, needs native review
queue-at-start = No hay elemento anterior.
# machine-drafted, needs native review
queue-missing = { $name } ya no está. Omitido.
# machine-drafted, needs native review
queue-network-not-played = Los elementos de red aún no se reproducen.
# machine-drafted, needs native review
queue-nothing-to-add = Nada que añadir. Solo los archivos de sonido, los vídeos y las listas van a la cola.
# machine-drafted, needs native review
queue-added = { $count ->
    [one] Se añadió 1 elemento a la cola.
   *[other] Se añadieron { $count } elementos a la cola.
}
# machine-drafted, needs native review
queue-added-next = { $count ->
    [one] 1 elemento sonará a continuación.
   *[other] { $count } elementos sonarán a continuación.
}
# machine-drafted, needs native review
queue-playlist-opened = { $name }: { $count ->
    [one] 1 elemento en la cola.
   *[other] { $count } elementos en la cola.
}
# machine-drafted, needs native review
queue-playlist-nothing = Nada de { $name } se puede reproducir aquí. La cola muestra lo que contiene.
# machine-drafted, needs native review
queue-playlist-failed = No se puede abrir { $name }: { $reason }
# machine-drafted, needs native review
queue-playlist-unreadable = no se puede leer el archivo.
# machine-drafted, needs native review
queue-playlist-damaged = no es una lista de reproducción que Specular sepa leer.
# machine-drafted, needs native review
queue-clear-ask = { $count ->
    [one] ¿Vaciar la cola (1 elemento)?
   *[other] ¿Vaciar la cola ({ $count } elementos)?
}
# machine-drafted, needs native review
queue-clear-detail = Solo se vacía la lista. Los archivos siguen en el disco.
# machine-drafted, needs native review
queue-clear-ok = Vaciar cola
# machine-drafted, needs native review
queue-cleared = Cola vaciada.
# machine-drafted, needs native review
queue-save-title = Guardar la cola como lista
# machine-drafted, needs native review
queue-default-name = Cola
# machine-drafted, needs native review
queue-format-m3u8 = Lista M3U8
# machine-drafted, needs native review
queue-format-xspf = Lista XSPF
# machine-drafted, needs native review
queue-saved = Se guardó { $name }.
# machine-drafted, needs native review
queue-save-failed = No se puede guardar { $name }: { $reason }
# machine-drafted, needs native review
settings-viewing-sound-play-next = Reproducir el siguiente archivo de sonido
# machine-drafted, needs native review
settings-viewing-sound-play-next-help = Cuando termina un archivo de sonido, suena el siguiente de la carpeta.
# machine-drafted, needs native review
settings-viewing-play-repeat = Repetir
# machine-drafted, needs native review
settings-viewing-play-repeat-help = Para archivos de sonido, vídeos y la cola.
# machine-drafted, needs native review
settings-viewing-play-repeat-off = No
# machine-drafted, needs native review
settings-viewing-play-repeat-all = Todo
# machine-drafted, needs native review
settings-viewing-play-repeat-one = Uno
# machine-drafted, needs native review
settings-viewing-play-shuffle = Aleatorio
# machine-drafted, needs native review
settings-viewing-play-shuffle-help = Reproducir en orden aleatorio, cada elemento una vez por ronda.
