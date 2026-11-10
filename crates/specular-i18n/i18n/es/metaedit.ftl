# The metadata editor: IPTC fields of the selected photos (gallery, Library, cull mode and
# the viewer).

palette-edit-metadata = Editar metadatos
palette-edit-metadata-help = Edita el título, el pie de foto, el creador, los derechos y las ubicaciones de las fotos seleccionadas.
metaedit-menu = Editar metadatos…
metaedit-title = Metadatos
metaedit-close = Cerrar metadatos
metaedit-photos = { $count ->
    [one] 1 foto
   *[other] { $count } fotos
}
metaedit-none-selected = Selecciona fotos para editar sus metadatos.
metaedit-reading = Leyendo metadatos…
metaedit-mixed = Mixto: escribe para aplicarlo a todas
metaedit-clearing = Borrado en todas las fotos. Esc los conserva.
metaedit-keywords-others = algunas tienen más
metaedit-saved = Se guardó { $field }.
metaedit-cleared = Se borró { $field }.
metaedit-write-failed = { $count ->
    [one] No se guardaron los metadatos de 1 foto: { $error }
   *[other] No se guardaron los metadatos de { $count } fotos: { $error }
}
metaedit-writes-sidecars = Guardado en archivos XMP adjuntos.
metaedit-writes-files = Guardado en archivos JPEG, TIFF y DNG; los demás archivos reciben archivos XMP adjuntos.

metaedit-group-templates = Plantillas
metaedit-group-description = Descripción
metaedit-group-creator = Creador
metaedit-group-rights = Derechos
metaedit-group-shown = Ubicación mostrada
metaedit-group-created = Ubicación de creación
metaedit-group-workflow = Flujo de trabajo
metaedit-templates-none = Aún no hay plantillas. Una plantilla rellena los campos que usas a menudo.

metaedit-field-title = Título
metaedit-field-headline = Titular
metaedit-field-description = Pie de foto
metaedit-field-keywords = Palabras clave (separadas por ;)
metaedit-field-persons-shown = Personas mostradas (separadas por ;)
metaedit-field-event = Evento
metaedit-field-creator = Creador (separados por ;)
metaedit-field-creator-job-title = Cargo del creador
metaedit-field-contact-address = Dirección
metaedit-field-contact-city = Ciudad
metaedit-field-contact-region = Estado o provincia
metaedit-field-contact-postal-code = Código postal
metaedit-field-contact-country = País
metaedit-field-contact-phone = Teléfono
metaedit-field-contact-email = Correo electrónico
metaedit-field-contact-url = Sitio web
metaedit-field-copyright = Aviso de copyright
metaedit-field-credit = Línea de crédito
metaedit-field-source = Fuente
metaedit-field-usage-terms = Condiciones de uso
metaedit-field-shown-sublocation = Sububicación
metaedit-field-shown-city = Ciudad
metaedit-field-shown-state = Estado o provincia
metaedit-field-shown-country = País
metaedit-field-shown-country-code = Código de país
metaedit-field-created-sublocation = Sububicación
metaedit-field-created-city = Ciudad
metaedit-field-created-state = Estado o provincia
metaedit-field-created-country = País
metaedit-field-created-country-code = Código de país
metaedit-field-instructions = Instrucciones
metaedit-field-job-id = ID de trabajo

metaedit-template-new = Nueva plantilla
metaedit-template-edit = Editar
metaedit-template-remove = Quitar
metaedit-template-done = Listo
metaedit-template-name = Nombre de la plantilla
metaedit-template-default-name = Plantilla { $n }
metaedit-template-title = Plantilla
metaedit-template-hint = Los campos vacíos no se modifican.
metaedit-template-replace-keywords = Reemplazar las palabras clave de las fotos
metaedit-template-applied = { $count ->
    [one] Se aplicó «{ $name }» a 1 foto.
   *[other] Se aplicó «{ $name }» a { $count } fotos.
}
metaedit-template-remove-ask = ¿Quitar la plantilla «{ $name }»?
metaedit-template-remove-keep = Las fotos conservan sus metadatos.
metaedit-template-remove-yes = Quitar
metaedit-template-removed = Se quitó la plantilla «{ $name }».

app-info-edit-metadata = Editar metadatos…

settings-gallery-metadata-into-files = Escribir los metadatos en los archivos de imagen
settings-gallery-metadata-into-files-help = El editor de metadatos también escribe en archivos JPEG, TIFF y DNG sin cambiar nada más en ellos. Desactivado: solo archivos XMP adjuntos.
