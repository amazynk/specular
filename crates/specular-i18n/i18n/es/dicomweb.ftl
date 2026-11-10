# Specular — DICOMweb servers, study search and streamed series (DESIGN.md §12.8). Español.

palette-dicom-servers = Servidores DICOMweb…

dicomweb-title = Servidores DICOMweb
dicomweb-servers-intro = Los estudios de un servidor solo se buscan y abren cuando usted lo pide. Antes no se envía nada.
dicomweb-servers = Servidores
dicomweb-servers-empty = Aún no hay servidores. Añada uno con su dirección DICOMweb.
dicomweb-session-only = Las contraseñas y los tokens solo se guardan hasta que se cierra Specular.
dicomweb-add = Añadir…
dicomweb-edit = Editar…
dicomweb-remove = Quitar
dicomweb-close = Cerrar
dicomweb-browse = Explorar
dicomweb-back = Atrás
dicomweb-save = Guardar
dicomweb-search = Buscar
dicomweb-stop = Detener
dicomweb-removed = Se quitó { $server }
# machine-drafted, needs native review
dicomweb-remove-ask = ¿Quitar «{ $server }» de los servidores guardados?
dicomweb-remove-detail = No se pierde nada del disco. Se olvida su contraseña o token de esta sesión.
# Servidores a los que se accede sin cifrado (http://).
dicomweb-not-encrypted = Sin cifrar

# Página para añadir o editar un servidor.
dicomweb-add-title = Añadir un servidor
dicomweb-edit-title = Ajustes del servidor
dicomweb-field-name = Nombre
dicomweb-hint-name = Opcional
dicomweb-field-url = Dirección
dicomweb-hint-url = https://pacs.example.org/dicom-web
dicomweb-field-auth = Inicio de sesión
dicomweb-auth-none = Sin inicio de sesión
dicomweb-auth-basic = Usuario y contraseña
dicomweb-auth-bearer = Token de acceso
dicomweb-field-user = Usuario
dicomweb-field-password = Contraseña
dicomweb-field-token = Token de acceso
dicomweb-hint-secret-kept = Déjelo vacío para conservar el actual
dicomweb-secret-session = La contraseña o el token solo se guarda hasta que se cierra Specular, nunca en el disco.
dicomweb-secret-needed = Vuelva a introducir la contraseña o el token: solo se guarda hasta que se cierra Specular.
dicomweb-http-warning = Esta dirección no está cifrada: la contraseña o el token se enviaría sin protección.
dicomweb-url-empty = Introduzca la dirección DICOMweb del servidor.
dicomweb-url-scheme = La dirección debe empezar por https:// o http://.
dicomweb-url-host = La dirección no tiene nombre de servidor.
dicomweb-url-userinfo = Ponga el usuario y la contraseña en los campos de inicio de sesión, no en la dirección.
dicomweb-save-failed = No se pudo guardar la lista de servidores: { $reason }

# Página de búsqueda.
dicomweb-search-title = Buscar estudios
dicomweb-field-patient-name = Nombre del paciente
dicomweb-field-patient-id = ID del paciente
dicomweb-field-date-from = Desde
dicomweb-field-date-to = Hasta
dicomweb-hint-date = AAAAMMDD
dicomweb-field-modality = Modalidad
dicomweb-hint-modality = CT, MR…
dicomweb-field-accession = Número de acceso
dicomweb-show-patient = Mostrar datos del paciente
dicomweb-switch-on = Activado
dicomweb-switch-off = Desactivado
dicomweb-patient-shown = Datos del paciente visibles
dicomweb-patient-hidden = Datos del paciente ocultos
dicomweb-studies = Estudios
dicomweb-search-hint = Rellene cualquiera de los campos y busque.
dicomweb-no-studies = Ningún estudio coincide.
dicomweb-searching = Buscando…
dicomweb-stopped = Detenido
dicomweb-load-more = Cargar más
dicomweb-study-untitled = Estudio sin descripción
dicomweb-series-untitled = Serie sin descripción
dicomweb-study-counts = { $series ->
    [one] Una serie
   *[other] { $series } series
}, { $images ->
    [one] una imagen
   *[other] { $images } imágenes
}
dicomweb-series-images = { $count ->
    [one] Una imagen
   *[other] { $count } imágenes
}
dicomweb-patient-id = ID { $id }
dicomweb-accession = N.º de acceso { $number }
dicomweb-found = { $count ->
    [one] Se encontró un estudio
   *[other] Se encontraron { $count } estudios
}
dicomweb-series-found = { $count ->
    [one] Una serie
   *[other] { $count } series
}

# Errores al comunicarse con un servidor.
dicomweb-error-offline = No se puede acceder a { $server }. Compruebe la red y la dirección.
dicomweb-error-auth = El servidor rechazó el usuario, la contraseña o el token.
dicomweb-error-not-found = No hay ningún servicio DICOMweb en esta dirección.
dicomweb-error-other = El servidor no pudo responder: { $reason }

# Serie transmitida al visor.
dicomweb-loading = Cargando { $series }…
dicomweb-loading-progress = Cargando { $series }: { $done } de { $total }
dicomweb-loaded = { $series }: { $total ->
    [one] una imagen cargada
   *[other] las { $total } imágenes cargadas
}
dicomweb-loaded-failed = No se pudieron cargar { $failed } de { $total } imágenes.
dicomweb-offline-cached = No se puede acceder a { $server }: se muestran las imágenes guardadas antes.
dicomweb-cache-failed = No se pudo abrir la carpeta de descargas: { $reason }
