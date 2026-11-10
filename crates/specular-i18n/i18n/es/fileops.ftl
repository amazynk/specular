# machine-drafted, needs native review
# Operaciones con archivos: cambiar nombre, cambiar nombre por lotes, copiar y mover a una carpeta,
# giro guardado en el archivo, deshacer y rehacer, fondo de pantalla, compartir, enviar por correo,
# exportar e importar la configuración.

## Paleta, hoja de referencia y menús

palette-undo = Deshacer
palette-undo-help = Revertir el último cambio en archivos: mover a la papelera, cambiar nombre, copiar o mover, girar.
palette-redo = Rehacer
palette-redo-help = Volver a hacer lo que revirtió el último Deshacer.
palette-rename = Cambiar nombre…
palette-rename-help = Dar un nombre nuevo al archivo; sus archivos auxiliares y su pareja RAW+JPEG lo siguen.
palette-batch-rename = Cambiar nombre de archivos…
palette-batch-rename-help = Cambiar el nombre de los archivos seleccionados, o de toda la carpeta, con un patrón y vista previa.
palette-copy-to-folder = Copiar a carpeta…
palette-copy-to-folder-help = Copiar el archivo o la selección a otra carpeta.
palette-move-to-folder = Mover a carpeta…
palette-move-to-folder-help = Mover el archivo o la selección a otra carpeta.
palette-rotate-file-left = Girar archivo a la izquierda
palette-rotate-file-left-help = Girar el propio archivo un cuarto a la izquierda, sin pérdida si el formato lo permite.
palette-rotate-file-right = Girar archivo a la derecha
palette-rotate-file-right-help = Girar el propio archivo un cuarto a la derecha, sin pérdida si el formato lo permite.
palette-set-wallpaper-help = Usar la imagen como fondo del escritorio.
palette-share = Compartir…
palette-share-help = Enviar el archivo a otra app con el menú Compartir del sistema.
palette-send-by-email = Enviar por correo…
palette-send-by-email-help = Crear un correo nuevo en tu app de correo con el archivo adjunto.
# machine-drafted, needs native review
palette-share-with-zynk = Compartir con Zynk…
palette-share-with-zynk-help = Crear un enlace web al archivo en la app de Zynk.
palette-send-with-zynk = Enviar con Zynk…
palette-send-with-zynk-help = Enviar el archivo a un contacto o a uno de tus dispositivos con Zynk.
palette-export-settings = Exportar configuración…
palette-export-settings-help = Guardar toda la configuración y tus atajos de teclado en un archivo.
palette-import-settings = Importar configuración…
palette-import-settings-help = Leer la configuración de un archivo, tras mostrar qué cambiaría.

disabled-in-archive-file = Esta página está dentro de un archivo comprimido y no tiene archivo propio
disabled-not-rotatable = Este tipo de archivo no puede guardar un giro
disabled-not-on-this-desktop = Tu escritorio no lo ofrece

## Configuración

settings-browsing-rotate-trim-edges = Recortar bordes JPEG para girar sin pérdida
settings-browsing-rotate-trim-edges-help = Un JPEG cuyo tamaño no es múltiplo de 8 o 16 píxeles solo puede girarse sin pérdida recortando sus últimos píxeles. Si no, se cambia su etiqueta de orientación y se te ofrece el recorte cada vez.
settings-browsing-rotate-keep-modified = Conservar la fecha de modificación al girar
settings-browsing-rotate-keep-modified-help = Un archivo girado conserva su fecha de último cambio, así no se mueve al ordenar por fecha.
settings-browsing-recent-folders = Carpetas recientes
settings-browsing-recent-folders-help = Últimas carpetas usadas para copiar y mover.

## Diálogos

fileop-cancel = Cancelar
rename-title = Cambiar nombre
rename-field-label = Nombre
rename-button = Cambiar nombre
rename-sidecars-follow = También cambian: { $files }
rename-extension-changed = Cambiar la extensión no convierte el archivo; otras apps quizá no lo abran.
rename-problem-empty = Escribe un nombre.
rename-problem-invalid-char = Los nombres no pueden contener «{ $char }».
rename-problem-reserved = «{ $name }» es un nombre que Windows reserva para dispositivos.
rename-problem-too-long = El nombre es demasiado largo; el máximo es 255 bytes.
rename-problem-dot = «.» y «..» no pueden usarse como nombres.
rename-problem-trailing = Los nombres no pueden terminar en espacio o punto.
rename-problem-exists = Ya hay otro archivo con este nombre.
rename-problem-read-only = Esta carpeta no se puede modificar.
rename-problem-duplicate = Dos archivos tendrían este nombre.

batch-rename-title =
    { $count ->
        [one] Cambiar nombre de { $count } archivo
       *[other] Cambiar nombre de { $count } archivos
    }
batch-rename-field-label = Patrón
batch-rename-tokens = Usa { "{" }name{ "}" }, { "{" }n:3{ "}" } (001, 002, …), { "{" }date:%Y-%m-%d{ "}" }, { "{" }camera{ "}" }, { "{" }w{ "}" }×{ "{" }h{ "}" } y { "{" }ext{ "}" }.
batch-rename-summary =
    { $count ->
        [one] { $count } archivo cambiará de nombre
       *[other] { $count } archivos cambiarán de nombre
    }{ $problems ->
        [0] .
        [one] ; { $problems } no puede y se queda igual.
       *[other] ; { $problems } no pueden y se quedan igual.
    }
batch-rename-reading = Leyendo fechas y cámaras…
batch-rename-preview = Vista previa
batch-rename-unchanged = conserva su nombre
batch-rename-more =
    { $count ->
        [one] { $count } más abajo
       *[other] { $count } más abajo
    }

transfer-copy-title-one = Copiar «{ $file }» a una carpeta
transfer-copy-title =
    { $count ->
        [one] Copiar { $count } archivo a una carpeta
       *[other] Copiar { $count } archivos a una carpeta
    }
transfer-move-title-one = Mover «{ $file }» a una carpeta
transfer-move-title =
    { $count ->
        [one] Mover { $count } archivo a una carpeta
       *[other] Mover { $count } archivos a una carpeta
    }
transfer-recent = Carpetas recientes
transfer-folder = Carpeta
transfer-choose-folder = Elegir carpeta…
transfer-copy-dialog = Copiar a carpeta
transfer-move-dialog = Mover a carpeta
transfer-sidecars =
    { $count ->
        [one] { $count } archivo auxiliar va con él.
       *[other] { $count } archivos auxiliares van con ellos.
    }
transfer-taken =
    { $count ->
        [one] { $count } nombre ya existe en { $folder }.
       *[other] { $count } nombres ya existen en { $folder }.
    }
transfer-when-taken = Si un nombre ya existe
transfer-skip = Omitir
transfer-keep-both = Conservar ambos
transfer-replace-option = Sustituir
transfer-copy = Copiar
transfer-move = Mover
transfer-replace-title =
    { $count ->
        [one] ¿Sustituir { $count } archivo en { $folder }?
       *[other] ¿Sustituir { $count } archivos en { $folder }?
    }
transfer-replace-note = Los archivos que hay allí van a la papelera. Deshacer los recupera.
transfer-replace = Sustituir
transfer-back = Atrás
transfer-stop = Detener

settings-file-import-title = Importar configuración
settings-file-import-dialog = Importar configuración
settings-file-export-dialog = Exportar configuración
settings-file-import-changes = Cambios
settings-file-import-change = { $before } → { $after }
settings-file-import-no-key = sin tecla
settings-file-import-unknown = No es un ajuste de esta versión; se omite.
settings-file-import-keymap = Atajos de teclado
settings-file-import-nothing = No cambiaría nada: esta configuración ya está aplicada.
settings-file-import-summary =
    { $count ->
        [one] { $count } cambio. La importación se puede deshacer en Configuración.
       *[other] { $count } cambios. La importación se puede deshacer en Configuración.
    }
settings-file-import-secrets =
    { $count ->
        [one] { $count } entrada parece una contraseña o clave y nunca se importa.
       *[other] { $count } entradas parecen contraseñas o claves y nunca se importan.
    }
settings-file-import-commands =
    { $count ->
        [one] { $count } comando de usuario no se importa: los comandos ejecutan programas. Cópialo tú en tu archivo de configuración si confías en él.
       *[other] { $count } comandos de usuario no se importan: los comandos ejecutan programas. Cópialos tú en tu archivo de configuración si confías en ellos.
    }
settings-file-import-button = Importar

## Resultados

fileop-renamed = Nombre cambiado a { $file }
fileop-renamed-count =
    { $count ->
        [one] { $count } archivo cambiado de nombre
       *[other] { $count } archivos cambiados de nombre
    }
fileop-copied =
    { $count ->
        [one] { $count } archivo copiado a { $folder }
       *[other] { $count } archivos copiados a { $folder }
    }
fileop-moved =
    { $count ->
        [one] { $count } archivo movido a { $folder }
       *[other] { $count } archivos movidos a { $folder }
    }
fileop-skipped =
    { $count ->
        [one] { $count } omitido: su nombre ya existe allí.
       *[other] { $count } omitidos: sus nombres ya existen allí.
    }
fileop-stopped = Detenido; lo hecho, hecho está.
fileop-failed-file =
    { $count ->
        [one] No se pudo copiar ni mover { $file }: { $reason }
       *[other] No se pudieron copiar ni mover { $count } archivos; el primero, { $file }: { $reason }
    }
fileop-progress-copy =
    { $count ->
        [one] Copiando { $count } archivo… { PERCENT($percent) }
       *[other] Copiando { $count } archivos… { PERCENT($percent) }
    }
fileop-progress-move =
    { $count ->
        [one] Moviendo { $count } archivo… { PERCENT($percent) }
       *[other] Moviendo { $count } archivos… { PERCENT($percent) }
    }
fileop-done-status = Hecho
fileop-busy = Aún se está aplicando el último cambio de archivos; inténtalo en un momento.
fileop-nothing-to-redo = Nada que rehacer
fileop-undone = Deshecho: { $what }
fileop-redone = Rehecho: { $what }
fileop-kind-rename = cambio de nombre
fileop-kind-batch-rename = cambio de nombre de varios archivos
fileop-kind-move = mover a carpeta
fileop-kind-copy = copiar a carpeta
fileop-kind-rotate = giro
rename-failed = No se pudo cambiar el nombre: { $reason }

undo-problem-changed = No se puede deshacer: { $file } cambió después y deshacer perdería ese cambio.
undo-problem-missing = No se puede deshacer: { $file } ya no está.
undo-problem-taken = No se puede deshacer: otro archivo se llama ahora { $file }.
undo-problem-io = No se puede deshacer el cambio en { $file }: { $reason }

rotate-done-lossless = { $file } girado sin ninguna pérdida.
rotate-done-lossless-trimmed = { $file } girado sin ninguna pérdida; su borde se recortó a { $width } × { $height }.
rotate-done-tag = { $file } girado cambiando su etiqueta de orientación.
rotate-done-sidecar = Giro de { $file } guardado en { $sidecar }; el archivo RAW nunca se modifica.
rotate-tag-unreadable = Sus datos de imagen no se pudieron leer lo suficiente para girarlo.
rotate-tag-multi-picture = Contiene más de una imagen, y girar la primera estropearía las demás.
rotate-tag-trailer = Tras la imagen hay datos que el giro desplazaría.
rotate-tag-not-aligned = Girar sus píxeles sin pérdida lo recortaría a { $width } × { $height }.
rotate-trim-offer = Recortar y girar
rotate-problem-unsupported = { $file } es un tipo de archivo que no puede guardar un giro.
rotate-problem-too-large = { $file } es demasiado grande para girarlo.
rotate-problem-damaged = { $file } está dañado donde guarda su orientación: { $reason }
rotate-problem-io = No se pudo girar { $file }: { $reason }

share-failed = No se pudo compartir: { $reason }
email-failed = No se pudo abrir tu app de correo: { $reason }
# machine-drafted, needs native review
zynk-opening = Abriendo Zynk…
zynk-failed = No se pudo conectar con Zynk: { $reason }
zynk-get-title-send = «Enviar con Zynk» necesita la app Zynk
zynk-get-title-share = «Compartir con Zynk» necesita la app Zynk
zynk-get-about = Zynk es una app gratuita para Mac, Windows, Linux, iPhone y Android. Envía fotos, vídeos y carpetas enteras directamente a tus otros dispositivos y a otras personas, del tamaño que sean. Las transferencias están cifradas de extremo a extremo y continúan donde se quedaron si se corta la conexión. Para un destinatario sin Zynk, crea un enlace web desde el que descargar los archivos en el navegador.
zynk-get-macos = Consíguela en la Mac App Store. Una vez instalada, { -app-name } le pasa tus archivos directamente.
zynk-get-windows = Consíguela en Microsoft Store. Una vez instalada, { -app-name } le pasa tus archivos directamente.
zynk-get-linux = Zynk para Linux es una AppImage, sin nada que instalar: descárgala, ponla en ~/Applications y hazla ejecutable (en sus Propiedades o con chmod +x). { -app-name } la encuentra ahí, y en cualquier otro sitio mientras Zynk está abierto.
zynk-get = Obtener Zynk
zynk-send-old-windows = «Enviar con Zynk» necesita Windows 10 versión 2004 o posterior. «Compartir con Zynk» funciona en este Windows.
settings-file-export-done = Configuración guardada en { $file }
settings-file-export-failed = No se pudo guardar la configuración: { $reason }
settings-file-import-unreadable = No se pudo leer { $file }: { $reason }
settings-file-import-done =
    { $count ->
        [one] { $count } cambio importado
       *[other] { $count } cambios importados
    }
