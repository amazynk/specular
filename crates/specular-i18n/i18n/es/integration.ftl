# Integration: open with, user commands, paste (Stage 2).
palette-open-with = Abrir con…
palette-paste = Pegar
open-with-placeholder = Abrir con…
open-with-loading = Buscando aplicaciones…
open-with-default = { $name } (predeterminada)
open-with-choose-other = Elegir otra aplicación…
open-with-choose-title = Elegir una aplicación
open-with-failed = No se pudo abrir con { $name }: { $reason }
command-missing = No se encontró { $program }
command-failed = No se pudo iniciar { $name }: { $reason }
command-exited = { $name } terminó con un error: { $status }
paste-nothing = No hay nada que abrir en el portapapeles
paste-failed = No se pudo pegar la imagen: { $reason }
drag-failed = No se pudo arrastrar el archivo: { $reason }
about-storage = Configuración: { $settings } · Caché: { $cache }
about-storage-portable = Modo portátil: ajustes y caché en { $dir }

## Scripting and recipes

palette-copy-view-command = Copiar vista como comando de shell
palette-copy-view-python = Copiar vista como Python
toast-copied-view-command = Comando copiado
script-copy-no-image = No hay imagen que describir
recipe-confirm-title = ¿Ejecutar la receta «{ $name }» por primera vez? Ejecuta: { $command }
recipe-confirm-run = Ejecutar
recipe-no-file = La receta necesita un archivo de imagen
recipe-running = Ejecutando { $name }…
recipe-done = { $name }: { $shapes ->
    [one] 1 forma añadida
   *[other] { $shapes } formas añadidas
}
recipe-failed = { $name } falló: { $reason }
recipe-timeout = detenido tras { $seconds } s
recipe-no-output = { $name } no escribió { $file }
