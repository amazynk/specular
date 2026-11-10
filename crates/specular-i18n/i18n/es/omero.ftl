# Specular — OMERO servers and the Image Data Resource (IDR), browsed read-only. Español. machine-drafted, needs native review

palette-omero-servers = Servidores OMERO…

omero-title = Servidores OMERO
omero-servers-intro = Los proyectos, cribados e imágenes de un servidor solo se listan cuando lo explora. Antes no se envía nada.
omero-servers = Servidores
omero-servers-empty = Aún no hay servidores. Añada uno con su dirección OMERO.web.
omero-session-only = Las contraseñas solo se guardan hasta que Specular se cierra.
omero-add = Añadir…
omero-edit = Editar…
omero-remove = Quitar
omero-close = Cerrar
omero-browse = Explorar
omero-back = Atrás
omero-save = Guardar
omero-stop = Detener
omero-open = Abrir
# $server a server's name.
omero-removed = Se quitó { $server }
# Shown with servers reached without encryption (http://).
omero-not-encrypted = Sin cifrar
# A server browsed without signing in.
omero-public = Público
# $user the user name a server is signed in with.
omero-signed-in-as = Como { $user }

# The page that adds or edits a server.
omero-add-title = Añadir un servidor
omero-edit-title = Ajustes del servidor
omero-field-name = Nombre
omero-hint-name = Opcional
omero-field-url = Dirección
omero-hint-url = https://omero.example.org
omero-field-user = Nombre de usuario
omero-hint-user = Vacío para datos públicos
omero-field-password = Contraseña
omero-hint-password-kept = Déjelo vacío para mantener la ya indicada
omero-password-session = La contraseña solo se guarda hasta que Specular se cierra, nunca en el disco.
omero-password-needed = Introduzca la contraseña: solo se guarda hasta que Specular se cierra.
omero-http-warning = Esta dirección no está cifrada: la contraseña se enviaría en claro.
omero-url-invalid = Introduzca la dirección OMERO.web, que empiece por https:// o http://.
# $reason why the list could not be saved.
omero-save-failed = No se pudo guardar la lista de servidores: { $reason }

# The browse page.
omero-browse-title = Explorar
omero-top = Proyectos y cribados
omero-datasets = Conjuntos de datos
omero-plates = Placas
omero-wells = Pocillos
omero-fields = Campos
omero-images = Imágenes
omero-kind-project = Proyecto
omero-kind-dataset = Conjunto de datos
omero-kind-image = Imagen
omero-kind-screen = Cribado
omero-kind-plate = Placa
omero-kind-well = Pocillo
# $well a well's row and column, as B7.
omero-well = Pocillo { $well }
omero-well-fields = { $count ->
    [one] Un campo
   *[other] { $count } campos
}
omero-children = { $count ->
    [one] Un elemento
   *[other] { $count } elementos
}
omero-load-more = Cargar más
omero-loading = Cargando…
omero-empty = Aquí no hay nada
omero-stopped = Detenido
omero-found = { $count ->
    [one] Un elemento
   *[other] { $count } elementos
}
# $reason why the image could not be opened.
omero-open-failed = No se pudo abrir la imagen: { $reason }
# The channel keys would hide the last channel shown.
omero-channel-last = Al menos un canal sigue visible
# $server a server's name.
omero-session-ended = La sesión en { $server } terminó. Introduzca la contraseña para continuar.

# Errors.
# $server a server's name.
omero-error-offline = No se pudo conectar con { $server }.
omero-error-sign-in = Este servidor requiere iniciar sesión.
omero-error-expired = La sesión terminó.
omero-error-login = Se rechazó el nombre de usuario o la contraseña.
omero-error-forbidden = Puede que su cuenta no tenga acceso a esto.
omero-error-not-found = No se encontró en el servidor (puede que se haya eliminado).
# $code an HTTP status code.
omero-error-status = El servidor respondió con el error { $code }.
# $reason what was wrong with the answer.
omero-error-malformed = No se pudo leer la respuesta del servidor: { $reason }
