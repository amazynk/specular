# machine-drafted, needs native review
# Remote control and events (Settings → Integration), and hooks: programs settings.toml
# names to run when something happens in the viewer.

settings-integration-ipc = Control remoto y eventos
settings-integration-ipc-help = Otros programas de este equipo, ejecutados con tu usuario, pueden controlar { -app-name } y saber qué ocurre en él a través de un socket privado (specular ipc). Nada escucha en la red.

## The hooks panel under "Remote control and events".
hooks-panel-title = Ganchos
hooks-panel-intro = Programas que settings.toml ejecuta cuando ocurre algo. Cada uno pregunta una vez antes de su primera ejecución.
hooks-panel-none = No hay ganchos. Añádelos como [[hooks]] en settings.toml.
hooks-panel-keywords = ganchos hooks eventos automatización scripts programas
# $events: the event names, joined ("marked, exported").
hooks-panel-on = Cuando { $events }
hooks-panel-untrusted = Pregunta antes de su primera ejecución
hooks-panel-allowed = Permitido
hooks-panel-remove = Quitar…
hooks-panel-unused = Sin uso

## Asking before a hook's first run (and again when its program or arguments change).
# $name: the hook's name; $program: the program; $events: what sets it off (hook-event-*,
# joined with hook-or).
hook-consent-title = ¿Ejecutar «{ $name }» ({ $program }) cuando { $events }?
hook-consent-detail = Inicia este programa con los nombres de las fotos: { $args }
hook-consent-trust = Permite solo programas de confianza. Vuelve a preguntar si el gancho cambia.
hook-consent-allow = Permitir
hook-consent-not-now = Ahora no
hook-or = { " " }o{ " " }
hook-event-opened = se abre una foto
hook-event-marked = se marcan fotos
hook-event-keywords = cambian las palabras clave
hook-event-exported = se exportan fotos
hook-event-imported = se importan fotos
hook-event-captured = una cámara envía una foto
hook-event-develop-saved = se guarda una edición de Revelar
hook-event-any = ocurre cualquier cosa

## Removing a hook (Settings › Integration › Hooks › Remove…).
hook-remove-title = ¿Quitar el gancho «{ $name }»?
hook-remove-detail = Se quita de settings.toml. El programa y tus archivos se quedan en el disco.
hook-remove-ok = Quitar
hook-removed = Se quitó el gancho «{ $name }».
hook-settings-unwritable = No se pudo cambiar settings.toml: { $reason }

## A hook that did not work (a quiet note; the log has the details).
hook-failed = El gancho «{ $name }» no funcionó: { $reason }
hook-failed-exit = terminó con el código { $code }
hook-failed-timeout = se detuvo tras { $seconds } s
hook-failed-start = no pudo iniciarse ({ $reason })
hook-dropped = El gancho «{ $name }» está ocupado: { $count ->
    [one] se omitió un evento.
   *[other] se omitieron { $count } eventos.
}

## An export started by another program (specular ipc export): nobody was asked.
export-recipes-remote-exists = { $count ->
    [one] ya existe un archivo que se reemplazaría: no se exportó nada
   *[other] ya existen { $count } archivos que se reemplazarían: no se exportó nada
}
export-recipes-remote-skipped = { $file } ya existe
