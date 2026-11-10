# machine-drafted, needs native review
## El panel del manual, los informes de fallo, el paquete de diagnóstico, las actualizaciones y Acerca de. Español.
## Estilo: breve, claro, sin relleno; una idea por frase.

## Paleta

palette-help-manual = Ayuda
palette-help-manual-help = Abrir el manual en la página de lo que hay en pantalla.
palette-save-diagnostics = Guardar paquete de diagnóstico…
palette-save-diagnostics-help = Guardar informes de fallo, el registro y detalles del sistema en un archivo zip. No se envía nada.
# Entrada de la paleta que abre una sección del manual; { $title } es una página o «Página › Sección».
palette-help-topic = Ayuda: { $title }
palette-help-topic-help = Abrir esta sección del manual.
# Bajo un comando no disponible en la paleta.
palette-disabled-help = No disponible ahora. Pulsa Intro para saber por qué.

## El panel de ayuda

help-title = Ayuda
help-search-placeholder = Buscar en el manual
help-contents = Contenido
help-back = Atrás
help-forward = Adelante
help-no-results = No se encontró nada para «{ $query }».
help-results = { $count ->
    [one] 1 sección encontrada
    [many] { $count } secciones encontradas
   *[other] { $count } secciones encontradas
}
help-section-start = Primeros pasos
help-section-modes = Visualización y modos
help-section-tasks = Cómo…
help-section-reference = Referencia
help-keys-unbound = Sin asignar
# Un comando citado en el manual sin tecla; se ejecuta desde la paleta.
help-key-unbound = { $name } (paleta)
help-keys-command = Comando
help-keys-keys = Teclas
help-commands = Todos los comandos están en la paleta de comandos ({ $key }). Escribe parte del nombre.
help-commands-no-key = Todos los comandos están en la paleta de comandos. Escribe parte del nombre.
help-link = Ir a { $title }
help-open = Ayuda
# Nombre del panel para lectores de pantalla; { $page } es la página mostrada.
help-label = Ayuda: { $page }
help-disabled-why = ¿Por qué?

## Informe de fallo ofrecido en el siguiente inicio

crash-title = { -app-name } se cerró inesperadamente
crash-body = Se guardó un informe en este equipo. No se envió nada.
crash-view = Ver informe
crash-hide = Ocultar informe
crash-copy = Copiar
crash-copied = Informe copiado
crash-save-bundle = Guardar paquete de diagnóstico…
crash-dismiss = Descartar

## Paquete de diagnóstico

diag-title = Guardar paquete de diagnóstico
diag-intro = Un archivo zip para adjuntar a un informe de errores. Se guarda en este equipo. No se envía nada.
diag-contents = Contenido
diag-include-names = Incluir nombres de archivo
diag-include-names-help = Desactivado: los nombres se sustituyen por file-1.jpg, dir-1, etc.
diag-privacy = Tu carpeta personal aparece como ~ y tu nombre de usuario como <user>. Nunca se incluyen contraseñas, direcciones de servidor ni imágenes.
diag-where = Informar de un problema: { $issues } · { $email }
diag-save = Guardar…
diag-cancel = Cancelar
diag-saved = Paquete de diagnóstico guardado: { $path }
diag-failed = No se pudo guardar el paquete de diagnóstico: { $reason }
diag-entry-summary = Versión, sistema, GPU y controlador
diag-entry-crash = Último informe de fallo
diag-entry-crashes = Informes de fallo anteriores ({ $count })
diag-entry-log = Últimas { $count } líneas del registro
diag-entry-settings = Configuración, sin contraseñas ni direcciones
diag-entry-gpu = Detalles de GPU y pantalla
diag-entry-readme = Qué es este paquete
diag-size = { $size }
# El README.txt dentro del paquete.
diag-readme =
    Paquete de diagnóstico de { -app-name }.

    Contiene lo que ayuda a encontrar un problema: versión y detalles del sistema, informes de fallo, el final del registro y la configuración.

    Tu carpeta personal aparece como ~ y tu nombre de usuario como <user>. Salvo que elijas incluirlos, los nombres de archivos y carpetas se sustituyen por file-1, dir-1, etc. Las contraseñas y las direcciones de servidor se eliminan.

    No se envió nada. Tú decides si compartes este archivo.

## Actualizaciones (Configuración)

settings-section-updates = Actualizaciones
settings-updates-check = Buscar actualizaciones
settings-updates-check-help = Una vez al día, preguntar si hay una versión nueva. La solicitud solo lleva el número de versión: sin identificador, sin cookie. Desactivado: { -app-name } nunca se conecta para actualizaciones.
settings-updates-channel = Canal
settings-updates-channel-help = Estable: solo versiones publicadas. Beta: también versiones de prueba.
settings-updates-channel-stable = Estable
settings-updates-channel-beta = Beta
settings-updates-check-now = Comprobar ahora
settings-updates-check-now-help = Comprobar una vez ahora.
update-available = La versión { $version } está disponible
update-notes = Notas de la versión
update-download = Descargar
update-none = { -app-name } está actualizado.
update-checking = Buscando actualizaciones…
update-failed = No se pudo buscar actualizaciones: { $reason }
update-last-checked = Última comprobación: { $when }

## Acerca de

about-build = Compilación { $commit } · { $profile } · { $target }
about-build-no-commit = Compilación { $profile } · { $target }
about-help = Ayuda
about-diagnostics = Guardar paquete de diagnóstico…
# machine-drafted, needs native review
about-renderer = Información del renderizador…
about-licences-count = { $count ->
    [one] 1 componente de terceros
    [many] { $count } componentes de terceros
   *[other] { $count } componentes de terceros
}
about-licence-group = { $licence } ({ $count })
about-contact = Incidencias: { $issues } · Soporte: { $email }
