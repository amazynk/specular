# Specular — Settings → Keys, the key binding editor (DESIGN.md §11). machine-drafted, needs native review
# Key names inside messages ($keys) are never translated.

settings-keys-sequence-timeout = Esperar la siguiente tecla
settings-keys-sequence-timeout-help = Cuánto espera una secuencia como G G su siguiente tecla. Una tecla que también empieza una secuencia más larga se ejecuta tras esta pausa.
settings-keys-bindings = Atajos de teclado
settings-keys-bindings-help = Seleccione una acción y pulse las teclas que quiera asignar. Cada modo tiene sus propias teclas; el modo de criba y las imágenes médicas recurren a las del visor. Para asignar Esc, púlsela brevemente; mantenerla cancela. Los atajos se guardan en keymap.toml.
palette-customise-keys = Personalizar teclas…
keys-mode-viewer = Visor
keys-mode-cull = Modo de criba
keys-mode-medical = Imágenes médicas
keys-modes = Modo
keys-presets = Preajuste
keys-preset-default = Predeterminado
keys-preset-vim = Vim
keys-preset-culling = Criba al estilo Lightroom
keys-preset-photo-mechanic = Al estilo Photo Mechanic
action-pan-left = Desplazar a la izquierda
action-pan-right = Desplazar a la derecha
action-pan-up = Desplazar hacia arriba
action-pan-down = Desplazar hacia abajo
action-pan-page-left = Desplazar una pantalla a la izquierda
action-pan-page-right = Desplazar una pantalla a la derecha
action-pan-page-up = Desplazar una pantalla hacia arriba
action-pan-page-down = Desplazar una pantalla hacia abajo
action-reload = Recargar la imagen
keys-action-cull-pick = Elegir
keys-action-cull-reject = Rechazar
keys-action-cull-unflag = Quitar la marca
keys-action-cull-rate = { $stars ->
    [0] Quitar la valoración
    [one] Valorar con 1 estrella
   *[other] Valorar con { $stars } estrellas
}
keys-action-cull-label = Etiqueta de color: { $label }
keys-action-cull-copy-to = Copiar a la carpeta { $slot }
keys-action-cull-move-to = Mover a la carpeta { $slot }
keys-import = Importar…
keys-export = Exportar…
keys-export-mode = Exportar { $mode }…
keys-reset-mode = Restablecer { $mode }
keys-reset-all = Restablecer todas las teclas
keys-add = Añadir una tecla para { $action }
keys-change = Cambiar { $keys }
keys-remove = Quitar { $keys }
keys-reset-action = Restablecer { $action }
keys-none = Sin teclas
keys-changed = Cambiado
keys-press = Pulse las teclas…
keys-recording-hint = Mantenga Esc para cancelar
keys-cancel-recording = Cancelar
keys-no-match = Ninguna acción ni tecla coincide con «{ $query }».
keys-list = Acciones y sus teclas
keys-confirm-preset = ¿Usar el preajuste { $preset }? Sustituye todos sus atajos.
keys-confirm-reset-mode = ¿Restablecer todas las teclas de { $mode } al preajuste?
keys-confirm-reset-all = ¿Restablecer todas las teclas de todos los modos al preajuste?
keys-confirm-ok = Sustituir
keys-confirm-reset = Restablecer
keys-confirm-cancel = Cancelar
keys-conflict-same = { $keys } ya ejecuta «{ $other }».
keys-conflict-prefix = { $keys } y { $sequence } («{ $other }») empiezan igual: la más corta solo se ejecuta tras una pausa.
keys-conflict-shadow = { $keys } ejecuta «{ $other }» en { $mode }.
keys-conflict-reserved = { $keys } puede que nunca llegue a Specular: { $reason }
keys-choice-replace = Sustituir
keys-choice-swap = Intercambiar
keys-choice-keep = Mantener ambas
keys-choice-anyway = Asignar de todos modos
keys-choice-cancel = Cancelar
keys-reserved-switch = cambia de ventana.
keys-reserved-close = cierra la ventana.
keys-reserved-secure = el sistema la usa.
keys-reserved-desktop = los atajos con Super pertenecen al escritorio.
keys-reserved-workspaces = cambia de terminal o de espacio de trabajo.
keys-reserved-input = cambia el método de entrada o la distribución del teclado.
keys-reserved-macos = macOS la usa.
keys-announce-recording = Grabando teclas para { $action }. Pulse las teclas o mantenga Esc para cancelar.
keys-announce-key = { $keys }
keys-announce-bound = { $keys } ahora ejecuta { $action }.
keys-announce-already = { $keys } ya ejecuta { $action }.
keys-announce-cancelled = Grabación cancelada.
keys-announce-removed = { $keys } quitada de { $action }.
keys-announce-reset-action = { $action } vuelve a tener las teclas del preajuste.
keys-announce-reset-mode = Las teclas de { $mode } se han restablecido.
keys-announce-reset-all = Todas las teclas se han restablecido.
keys-announce-preset = Ahora se usa el preajuste { $preset }.
keys-toast-imported = Atajos importados
keys-toast-exported = Atajos exportados
keys-toast-reloaded = keymap.toml ha cambiado: atajos recargados
keys-toast-error = No se pudieron leer los atajos: { $error }
keys-toast-write-error = No se pudo guardar keymap.toml: { $error }
keys-pending = { $keys } …  { $next }
keys-problem-syntax = No se pudo leer keymap.toml: { $error }
keys-problem-key = Se ignoró la tecla desconocida «{ $key }».
keys-problem-action = «{ $key }» nombra una acción desconocida «{ $action }»; se ignoró.
keys-problem-mode = La tabla [{ $mode }] no es un modo; se ignoró.
keys-problem-duplicate = { $keys } aparece dos veces en { $mode }; se usa la segunda.
keys-problem-prefix = { $keys } empieza { $longer } en { $mode }, así que solo se ejecuta tras una pausa.
keys-problem-preset = Preajuste desconocido «{ $preset }»; se aplican las teclas predeterminadas.

## keymap.toml editor (machine-drafted, needs native review)

keys-edit-file = Editar keymap.toml
keys-problem-action-suggestion = «{ $key }» nombra una acción desconocida «{ $action }»; se ignoró. ¿«{ $suggestion }»uería decir «{ $suggestion }»?
keys-problem-mode-suggestion = La tabla [{ $mode }] no es un modo; se ignoró. ¿[{ $suggestion }]uería decir [{ $suggestion }]?
keys-problem-preset-suggestion = Preajuste desconocido «{ $preset }»; se aplican las teclas predeterminadas. ¿«{ $suggestion }»uería decir «{ $suggestion }»?
