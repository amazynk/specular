# machine-drafted, needs native review
# Specular — Develop: historial, instantáneas, estilos, ajustes preestablecidos, versiones, copiar y pegar, igualar. Español.

palette-develop-copy = Copiar edición
palette-develop-copy-help = Copiar la edición de la foto, salvo recorte y objetivo, para pegarla en otras fotos.
palette-develop-copy-some = Copiar algunos módulos…
palette-develop-copy-some-help = Elegir qué módulos de la edición copiar.
palette-develop-paste = Pegar edición
palette-develop-paste-help = Pegar la edición copiada en la foto, o en las fotos seleccionadas en la galería.
palette-develop-paste-some = Pegar algunos módulos…
palette-develop-paste-some-help = Elegir qué módulos copiados pegar, añadiéndolos o reemplazando.
palette-develop-paste-picks = Pegar edición en las elegidas
palette-develop-paste-picks-help = Pegar la edición copiada en cada foto elegida.
palette-develop-reference = Usar como referencia
palette-develop-reference-help = Otras fotos podrán igualar la exposición y el balance de blancos de esta.
palette-develop-match = Igualar a la referencia
palette-develop-match-help = Ajustar la exposición y el balance de blancos de la foto, la selección o las elegidas a la referencia.
palette-develop-sync = Sincronizar con las elegidas
palette-develop-sync-help = Poner la edición de esta foto, salvo recorte y objetivo, en las fotos elegidas.
palette-develop-sync-some = Sincronizar algunos módulos…
palette-develop-sync-some-help = Elegir qué módulos de esta foto van a las fotos elegidas.
palette-develop-next-capture = Estilo de las próximas tomas…
palette-develop-next-capture-help = El estilo que recibe cada archivo nuevo que llega a la carpeta.
palette-develop-auto-rules = Reglas automáticas…
palette-develop-auto-rules-help = Las reglas que aplican un estilo la primera vez que una foto se abre en Develop.
palette-develop-import-style = Importar estilo…
palette-develop-import-style-help = Añadir un estilo desde un archivo.
palette-develop-snapshot = Tomar instantánea
palette-develop-snapshot-help = Poner nombre a la edición tal como está, para compararla o volver a ella más tarde.
palette-develop-compress = Compactar historial
palette-develop-compress-help = Quitar pasos que otros posteriores reemplazaron. La edición no cambia.
palette-develop-new-version = Nueva versión
palette-develop-new-version-help = Otra edición de la misma foto, con su propio historial. El archivo no se copia.
palette-develop-new-style = Nuevo estilo…
palette-develop-new-style-help = Guardar módulos elegidos de la edición como estilo.

develop-styles = Estilos
develop-snapshots = Historial e instantáneas
develop-versions = Versiones
develop-style-new = Nuevo estilo a partir de esta edición…
develop-style-import = Importar estilo…
develop-rules = Reglas automáticas…
develop-snapshot-take = Tomar instantánea
develop-history-compress = Compactar historial
develop-snapshot-comparing = Comparando: { $name }
develop-snapshot-compare-off = Comparación desactivada
develop-version-number = Versión { $number }
develop-version-open = { $name } (abierta)
develop-version-new = Nueva versión
develop-version-duplicate = Duplicar esta versión

develop-history-paste = Pegar
develop-history-sync = Sincronizar
develop-history-match = Igualar a la referencia
develop-history-snapshot = Instantánea: { $name }
develop-history-previous = Igual que la anterior

develop-rename = Renombrar…
develop-delete = Eliminar
develop-save = Guardar
develop-name = Nombre
develop-style-apply = Aplicar
develop-style-apply-replace = Aplicar reemplazando estos módulos
develop-style-auto = Aplicar a fotos nuevas de esta cámara
develop-style-export = Exportar…
develop-snapshot-restore = Volver a esta instantánea
develop-version-open-it = Abrir
develop-preset-save = Guardar como ajuste…
develop-preset-save-module = Guardar { $module } como ajuste…
develop-preset-delete = Eliminar «{ $name }»…

develop-sheet-modules = Módulos
develop-sheet-included = Incluido
develop-sheet-left-out = Excluido
develop-sheet-more = { $count } más
develop-copy-title = Copiar módulos
develop-copy-ok = Copiar
develop-paste-title = Pegar módulos
develop-paste-ok = Pegar
develop-paste-mode = Módulos que ya están en la edición
develop-paste-append = Añadir
develop-paste-replace = Reemplazar
develop-style-new-title = Nuevo estilo
develop-style-save = Guardar estilo
develop-style-default-name = Mi estilo { $number }
develop-style-rename-title = Renombrar estilo
develop-preset-save-title = Ajuste de { $module }
develop-preset-default-name = Mi ajuste
develop-snapshot-take-title = Tomar instantánea
develop-snapshot-default-name = Instantánea { $number }
develop-snapshot-rename-title = Renombrar instantánea
develop-version-new-title = Nueva versión
develop-version-duplicate-title = Duplicar versión
develop-version-create = Crear
develop-version-rename-title = Renombrar versión

develop-style-delete-ask = ¿Eliminar el estilo «{ $name }»?
develop-style-delete-detail = Las fotos donde se aplicó conservan su edición. Se elimina el archivo del estilo.
develop-preset-delete-ask = ¿Eliminar el ajuste «{ $name }»?
develop-preset-delete-detail = Las ediciones donde se aplicó quedan como están.
develop-snapshot-delete-ask = ¿Eliminar la instantánea «{ $name }»?
develop-snapshot-delete-detail = La edición y su historial quedan como están.
develop-version-delete-ask = ¿Eliminar «{ $name }»?
develop-version-delete-detail = Se eliminan la edición y el historial de esta versión. La foto y sus otras versiones siguen en el disco.
develop-rule-delete-ask = ¿Eliminar la regla «{ $rule }»?
develop-rule-delete-detail = El estilo se queda, y las fotos ya abiertas conservan su edición.

develop-rule-text = { $style }: { $when }
develop-rule-every-photo = todas las fotos
develop-rule-iso-min = ISO { $iso } o más
develop-rule-iso-max = ISO hasta { $iso }
develop-rule-iso-range = ISO { $low }–{ $high }
develop-rule-raw = archivos raw
develop-rule-other = no raw
develop-rule-any-camera = cualquier cámara
develop-next-placeholder = Estilo de cada archivo nuevo
develop-next-off-row = Ninguno
develop-next-previous-row = Igual que la foto anterior

develop-copied = { $count ->
    [one] 1 módulo copiado de { $name }
   *[other] { $count } módulos copiados de { $name }
}
develop-pasted = { $count ->
    [one] Pegado en 1 foto
   *[other] Pegado en { $count } fotos
}
develop-nothing-to-copy = No hay foto de la que copiar.
develop-nothing-copied = Aún no hay nada copiado. Copia primero una edición (Ctrl+Alt+C).
develop-no-photos = No hay fotos que cambiar.
develop-no-picks = No hay fotos elegidas.
develop-reference-set = Referencia: { $name }
develop-no-reference = Aún no hay referencia. Abre la foto de referencia y elige Usar como referencia (Ctrl+Alt+R).
develop-reference-unreadable = No se pudo leer la foto de referencia.
develop-matching = { $count ->
    [one] Igualando 1 foto…
   *[other] Igualando { $count } fotos…
}
develop-matched = { $count ->
    [one] 1 foto igualada
   *[other] { $count } fotos igualadas
}
develop-matched-some = { $count } igualadas. { $failed } no se pudieron leer o medir.
develop-style-saved = Estilo guardado: { $name }
develop-style-applied = Estilo aplicado: { $name }
develop-style-imported = Estilo importado: { $name }
develop-style-not-a-style = Este archivo no es un estilo.
develop-style-exported = Estilo exportado: { $name }
develop-style-export-failed = No se pudo escribir el estilo ahí.
develop-style-import-dialog = Importar un estilo
develop-style-export-dialog = Exportar el estilo
develop-style-file-kind = Estilo
develop-preset-saved = Ajuste guardado: { $name }
develop-preset-applied = Ajuste aplicado: { $name }
develop-snapshot-taken = Instantánea tomada: { $name }
develop-history-compressed = { $count ->
    [0] Nada que compactar
    [one] 1 paso quitado
   *[other] { $count } pasos quitados
}
develop-version-created = Versión creada: { $name }
develop-version-opened = Versión abierta: { $name }
develop-version-deleted = Versión eliminada: { $name }
develop-rule-added = { $name } se aplica a fotos nuevas de { $camera }.
develop-rule-applied = Regla automática: { $name }
develop-rules-none = No hay reglas automáticas. Añade una desde el menú de un estilo.
develop-next-off = Los archivos nuevos no reciben estilo.
develop-next-previous = Los archivos nuevos reciben la edición de la foto anterior.
develop-next-style = Los archivos nuevos reciben el estilo { $name }.
develop-next-applied = { $count ->
    [one] Estilo de próximas tomas en 1 archivo nuevo
   *[other] Estilo de próximas tomas en { $count } archivos nuevos
}

develop-style-specular = Specular intenso
develop-style-soft = Suave
develop-style-vivid = Vivo
develop-style-mono = Blanco y negro
develop-preset-plus-one = Más claro (+1 EV)
develop-preset-minus-one = Más oscuro (−1 EV)
develop-preset-low-contrast = Contraste bajo
develop-preset-high-contrast = Contraste alto
develop-preset-open-shadows = Abrir sombras
develop-preset-crisp = Nítido
develop-preset-muted = Apagado
develop-preset-high-iso = ISO alto
develop-preset-screen = Para pantallas

gallery-badge-versions = { $count } versiones
# machine-drafted, needs native review
develop-batch-row = Guardando ediciones
# machine-drafted, needs native review
develop-batch-row-detail = { $done } de { $total } fotos
# machine-drafted, needs native review
develop-batch-saving = Guardando
# machine-drafted, needs native review
develop-batch-stopping = Deteniendo
# machine-drafted, needs native review
develop-batch-stopped = Detenido. Se guardaron { $done } de { $total } fotos; el resto no cambia.
