# Specular — Local AI pack (DESIGN.md "Local AI pack", §10). Spanish. Machine-drafted, needs native review.

## Settings → Local AI
settings-section-ai = IA local
settings-ai-pack = Paquete de IA local
# machine-drafted, needs native review
settings-ai-pack-help = Funciones opcionales que se ejecutan solo en este ordenador. Nada de lo que encuentran sale de él. Elige un modelo para cada función; no se descarga nada hasta que lo eliges.
# machine-drafted, needs native review
settings-ai-install-from-file = Instalar desde archivo…
settings-ai-install-from-file-help = Instala archivos de modelo que copiaste a este equipo, para equipos sin internet. Cada archivo se comprueba con la lista incluida en { -app-name }.
settings-ai-install-from-folder = Instalar desde carpeta…
settings-ai-install-from-folder-help = Lo mismo, para cada archivo de modelo de una carpeta.
settings-ai-search = Búsqueda por descripción
settings-ai-search-help = Encuentra fotos describiéndolas, como «perro en una playa», y agrupa mejor las tomas parecidas.
settings-ai-text = Texto en imágenes
settings-ai-text-help = Lee el texto de fotos y documentos escaneados para que la búsqueda text: los encuentre.
settings-ai-culling = Ayuda para cribar
# machine-drafted, needs native review
settings-ai-culling-help = Marca los ojos cerrados, el desenfoque y la mala exposición para ayudar a elegir la mejor toma de una ráfaga.
settings-ai-people = Personas
# machine-drafted, needs native review
settings-ai-people-help = Encuentra caras y las agrupa por persona, para la búsqueda people:. La agrupación está desactivada hasta que la actives en una biblioteca. Los datos faciales se pueden eliminar de una persona o de todas a la vez.

# The pack's and a feature's state. $percent is formatted ("25%"); $done and $total are
# formatted numbers.
ai-state = { $state ->
        [none] No hay modelos disponibles en esta compilación.
        [installed] Instalado
        [indexing] Instalado · { $done } de { $total } imágenes indexadas
        [downloading] Descargando… { $percent }
        [failed] Error en la descarga
       *[available] No instalado
    }
ai-removing = Eliminando…
# machine-drafted, needs native review
# Buttons of the Local AI page.
ai-button = { $action ->
        [download] Descargar
        [cancel] Cancelar
        [remove] Quitar
        [forget] Olvidar
        [faces] Eliminar todos los datos faciales
       *[pack] Quitar paquete
    }
# The question before deleting.
ai-confirm = { $action ->
        [faces] ¿Eliminar todos los datos faciales? Se eliminan todas las caras y personas encontradas. Tus fotos se conservan.
       *[pack] ¿Quitar el paquete de IA local, sus modelos y todo lo que ha encontrado? Tus fotos no se modifican.
    }

## Gallery search
gallery-query-needs-ai = { $field }: necesita el paquete de IA local (Configuración › IA local)
# machine-drafted, needs native review
gallery-query-ai-not-yet = { $field }: el paquete de IA local aún no puede buscar esto

## Faces: People view, face boxes and culling flags
# machine-drafted, needs native review
# "Show faces" on the picture.
faces-shown = Caras visibles
# machine-drafted, needs native review
faces-hidden = Caras ocultas
# machine-drafted, needs native review
# Beside a face box whose eyes look closed.
faces-eyes-closed = Ojos cerrados
# machine-drafted, needs native review
# Culling flags on the cull mode strip.
cull-flag-eyes-closed = Ojos cerrados
# machine-drafted, needs native review
cull-flag-eyes-closed-some = Ojos cerrados: { $closed } de { $faces } caras
# machine-drafted, needs native review
cull-flag-blurred = Borrosa
# machine-drafted, needs native review
cull-flag-dark = Demasiado oscura
# machine-drafted, needs native review
cull-flag-bright = Demasiado clara
# machine-drafted, needs native review
cull-flag-clipped = Altas luces o sombras recortadas
# machine-drafted, needs native review
# The People view.
people-title = Personas
# machine-drafted, needs native review
people-open = Personas
# machine-drafted, needs native review
people-done = Listo
# machine-drafted, needs native review
people-back = Atrás
# machine-drafted, needs native review
people-show-photos = Mostrar fotos
# machine-drafted, needs native review
people-name = Nombrar…
# machine-drafted, needs native review
people-name-prompt = Ponle nombre a esta persona
# machine-drafted, needs native review
people-faces-open = Caras
# machine-drafted, needs native review
people-faces-of = Caras de { $name }
# machine-drafted, needs native review
people-merge = Combinar
# machine-drafted, needs native review
people-merged = { $count ->
        [one] { $count } persona combinada
       *[other] { $count } personas combinadas
    }
# machine-drafted, needs native review
people-merge-ask = ¿Unir { $from } con { $into }?
# machine-drafted, needs native review
people-merge-ask-many = ¿Combinar { $count } personas con { $into }?
# machine-drafted, needs native review
people-merge-ask-detail = Sus caras pasan a ser de { $into }. Las fotos del disco no cambian.
# machine-drafted, needs native review
people-not-this = No es esta persona
# machine-drafted, needs native review
people-face-removed = Cara quitada de esta persona. La foto se conserva.
# machine-drafted, needs native review
# Someone not named yet.
people-unnamed = Persona #{ $id }
# machine-drafted, needs native review
people-face-count = { $count ->
        [one] { $count } cara
       *[other] { $count } caras
    }
# machine-drafted, needs native review
people-count = { $count ->
        [one] { $count } persona
       *[other] { $count } personas
    }
# machine-drafted, needs native review
people-face-a11y = Cara { $index }
# machine-drafted, needs native review
people-loading = Cargando…
# machine-drafted, needs native review
people-not-installed = Personas no está instalado. Descárgalo en Configuración › IA local.
# machine-drafted, needs native review
people-grouping-off = La agrupación de caras está desactivada en esta biblioteca. Activa «Agrupar caras en personas» en Configuración › IA local.
# machine-drafted, needs native review
people-none = Aún no se han encontrado personas.

# machine-drafted, needs native review
# A feature's model: Default (the model Specular ships as the default, or the nearest one
# this computer can run) or a model by name, which stays that model on any computer. The
# page shows it only for a feature with more than one model; the dropdown labels Default
# with the model it means (ai-size-recommended-option) and names each model as the model
# list does. The option names below are the palette's and the command line's: model names
# are proper names, kept as they are.
settings-ai-search-size = Modelo
settings-ai-search-size-help = SigLIP 2 B/16 encuentra mejor las fotos, en muchos idiomas. B/32 indexa unas cuatro veces más rápido, para tarjetas gráficas más lentas.
settings-ai-search-size-recommended = Predeterminado
# machine-drafted, needs native review
settings-ai-search-size-small = SigLIP 2 B/32
settings-ai-search-size-standard = PE Core S16
settings-ai-search-size-large = SigLIP 2 B/16
settings-ai-text-size = Modelo
settings-ai-text-size-help = PP-OCRv6 small lee con más precisión; tiny es más rápido.
settings-ai-text-size-recommended = Predeterminado
settings-ai-text-size-small = PP-OCRv6 tiny
settings-ai-text-size-standard = PP-OCRv6 small
settings-ai-text-size-large = Grande
# machine-drafted, needs native review
settings-ai-culling-size = Modelo
settings-ai-culling-size-help = El modelo que detecta ojos cerrados.
settings-ai-culling-size-recommended = Predeterminado
settings-ai-culling-size-small = Pequeño
# machine-drafted, needs native review
settings-ai-culling-size-standard = Face Landmarker
settings-ai-culling-size-large = Grande
# machine-drafted, needs native review
settings-ai-people-size = Modelo
settings-ai-people-size-help = SFace es pequeño. AuraFace es más grande y está entrenado con datos que su editor dice que tienen licencia para uso comercial.
settings-ai-people-size-recommended = Predeterminado
settings-ai-people-size-small = Pequeño
# machine-drafted, needs native review
settings-ai-people-size-standard = BlazeFace + SFace
settings-ai-people-size-large = BlazeFace + AuraFace
# A model size by name.
ai-tier = { $tier ->
        [small] Pequeño
        [large] Grande
       *[standard] Estándar
    }
# machine-drafted, needs native review
# Default in the model dropdown, and the label of the "Back to default" row: with the model
# it means on this computer, "Default (SigLIP 2 B/16)". $size is the model's name.
ai-size-recommended-option = Predeterminado ({ $size })
# Under the model setting: the default, or the nearest model this computer can run. $gpu is
# the graphics card's name.
ai-recommended-for = Predeterminado para tu { $gpu }: { $size }
ai-recommended = Predeterminado para este ordenador: { $size }
# Nothing fits: no size can run here.
ai-recommended-none = Ningún tamaño cabe en la memoria de este equipo.
# Under an installed feature: what its model runs on now. $gpu is the graphics card's name
# (used with "gpu" only). The CPU variants say why the graphics card is not used.
ai-runs-on = { $engine ->
        [gpu] Se ejecuta en: GPU ({ $gpu })
        [gpu-unnamed] Se ejecuta en: GPU
        [no-gpu] Se ejecuta en: CPU (no se encontró GPU)
        [differed] Se ejecuta en: CPU: el resultado de la GPU difería del de la CPU
        [failed] Se ejecuta en: CPU: el modelo falló en la GPU
        [setting] Se ejecuta en: CPU (el procesamiento está en «Solo la CPU»)
        [checking] Se ejecuta en: CPU: comprobando la GPU con ella
       *[cpu] Se ejecuta en: CPU
    }
# A size's row: its files on disk ($place "disk"), or what it would download, the memory it
# needs, and " · needs the GPU" only for a size that cannot run without one. $bytes and
# $memory are formatted sizes.
ai-size-detail = { $place ->
        [disk] { $bytes } en disco
       *[download] { $bytes } de descarga
    } · { $memory } de memoria{ $gpu ->
        [yes] { " · necesita la GPU" }
       *[no] { "" }
    }
# machine-drafted, needs native review
# Under a size that may not run on this computer (it is still shown, and choosing it asks
# first). "gpu": it needs a graphics card this computer does not have or cannot use;
# "memory": $need is the memory it needs, $have what this computer has for it (formatted
# sizes, "6 GB").
ai-size-unfit = { $why ->
        [gpu] Necesita una GPU y este equipo no tiene ninguna que pueda usar.
       *[memory] Necesita { $need } de memoria, más de los { $have } que Specular permite para la IA en este equipo.
    }
# Over the size setting while it names a size other than the one installed and in use
# ($current, $chosen: ai-tier). The rows below have the buttons to change it.
ai-size-not-in-use = Se está usando { $current }, no { $chosen }.
# machine-drafted, needs native review
# Buttons of a size's row and of the size question: "use" chooses that size (one not
# downloaded; a downloaded one says Switch, ai-size-button), "back" returns the setting to
# Recommended, "anyway" keeps a size that may not run on this computer (ai-unfit-question).
# The screen reader adds the size's name to each.
ai-size-action = { $action ->
        [back] Volver al predeterminado
        [anyway] Usar igualmente
       *[use] Usar este tamaño
    }
# The size the setting chose is not installed while another one is ($current, ai-tier).
ai-size-chosen-missing = Elegido, pero aún sin descargar: se sigue usando el tamaño { $current }.
# What a size is now: from what is installed, never from the setting. "downloaded": its files
# are here but another size is in use; "missing": the chosen size, not downloaded.
ai-size-state = { $state ->
        [current] En uso
        [downloaded] Descargado, sin usar
        [missing] Sin descargar
        [previous] En uso hasta que el nuevo tamaño haya indexado tu biblioteca
        [target] Descargando…
        [failed] Error en la descarga
       *[other] { "" }
    }
# A licence row's button (opens the licence's page).
ai-licence = Ver licencia
# Asked before a size change that downloads and re-indexes. $feature is the feature's name,
# $size the new size (ai-tier), $download a formatted size.
ai-switch-question = { $download ->
        [none] { $feature }: ¿cambiar a { $size }? Tu biblioteca se volverá a indexar; hasta entonces se sigue usando el tamaño actual.
       *[other] { $feature }: ¿cambiar a { $size }? Se descargarán { $download } y tu biblioteca se volverá a indexar; hasta entonces se sigue usando el tamaño actual.
    }
# machine-drafted, needs native review
# A size that may not run on this computer, chosen for a feature not installed yet: asked
# before the setting keeps it (ai-size-unfit says why, on the next line). $size is ai-tier,
# $feature the feature's name.
ai-unfit-question = { $feature }: ¿usar { $size } igualmente? Puede que no funcione en este equipo, o que vaya muy lento.
# The recommendation moved: offered, never applied silently.
ai-suggest-question = { $feature }: { $size } se adapta ahora mejor a este equipo. ¿Cambiar? { $download ->
        [none] Tu biblioteca se volverá a indexar.
       *[other] Se descargarán { $download } y tu biblioteca se volverá a indexar.
    }
ai-suggest-toast = IA local: otro tamaño de modelo se adapta mejor a este equipo (Configuración › IA local).
# A feature switching size: the new size ($size, ai-tier) indexes the library while the old one
# stays in use. $done and $total are formatted numbers.
ai-switching = Cambiando a { $size } · { $done } de { $total } imágenes indexadas

## The model manager: what each model says besides its size and licence

# machine-drafted, needs native review
# A model or feature not installable yet.
ai-coming = Disponible en una versión posterior
# machine-drafted, needs native review
# What a model reads. $languages: multilingual, english, or a script id (OCR packs).
ai-choice-languages = { $languages ->
        [multilingual] Muchos idiomas
        [english] Solo inglés
        [cjk-latin] Chino, japonés y alfabeto latino
        [zh-latin] Chino y alfabeto latino
        [arabic] Árabe
        [hebrew] Hebreo
        [korean] Coreano
        [cyrillic] Cirílico
        [greek] Griego
        [thai] Tailandés
        [devanagari] Devanagari
        [tamil] Tamil
        [telugu] Telugu
        [eslav] Ruso, ucraniano y bielorruso
        [latin] Alfabeto latino (muchos idiomas europeos)
       *[other] { $languages }
    }
# A hint about a model. $hint: faster, closed-eyes, small-faces.
ai-choice-hint = { $hint ->
        [faster] Más rápido, para tarjetas gráficas más lentas
        [closed-eyes] Detecta ojos cerrados
        [small-faces] Mejor con caras pequeñas
        [no-translate] No traduce al inglés
       *[other] { "" }
    }
# A model's training data. $source: the data as its publisher names it ($named is "no" when
# it is not named). $terms: whether the publisher states the data's terms ("stated"), does
# not ("unstated"), or the data is licensed for research only ("research").
ai-choice-data = { $terms ->
        [stated] Datos de entrenamiento: { $source }
        [research] Datos de entrenamiento: { $source } (licencia solo para investigación)
       *[unstated] { $named ->
            [no] Datos de entrenamiento: sin nombre, condiciones no indicadas
           *[yes] Datos de entrenamiento: { $source }, condiciones no indicadas
        }
    }
# Where a model downloads from. $host is a host name ("huggingface.co").
ai-choice-host = Se descarga de { $host }
# A model still coming, in a feature's row. $size is formatted, $licence an SPDX name.
ai-choice-coming = { $name }: { $size }, { $licence }
# People: grouping faces into people, per library.
ai-grouping = Agrupar caras en personas
ai-grouping-help = Desactivado hasta que lo actives en esta biblioteca. Al desactivarlo se deja de agrupar; las caras ya agrupadas se quedan hasta que las elimines.
ai-grouping-state = { $on ->
        [yes] Activado en esta biblioteca
       *[no] Desactivado
    }
ai-grouping-button = { $on ->
        [yes] Desactivar
       *[no] Activar
    }
# machine-drafted, needs native review
# An add-on (a script's text recogniser) before its feature is downloaded. $feature is the
# feature's name ("Text in photos").
ai-addon-first = Descarga primero { $feature }.
# machine-drafted, needs native review
# Asked before an add-on is removed. $name is the model's name ("PP-OCRv5 Arabic").
ai-addon-remove-title = ¿Quitar { $name }?
# machine-drafted, needs native review
ai-addon-remove-detail = Se eliminan sus archivos de modelo. Tus fotos y el texto ya encontrado se conservan.
# machine-drafted, needs native review
# Text: reading the text in this library's photos, per library.
ai-text-indexing = Leer el texto de las fotos de esta biblioteca
# machine-drafted, needs native review
ai-text-indexing-help = Desactivado hasta que lo actives en esta biblioteca. Después, text: en la búsqueda de la Biblioteca encuentra fotos por las palabras que contienen.
# machine-drafted, needs native review
ai-text-indexing-state = { $on ->
        [yes] Activado en esta biblioteca
       *[no] Desactivado
    }

## Instalar desde archivo

# machine-drafted, needs native review
# After model files were copied in.
ai-sideload-ready = { $count ->
        [one] Se copió un archivo de modelo. Pulsa «Descargar» para instalarlo.
        [many] Se copiaron { $count } archivos de modelo. Pulsa «Descargar» para instalarlos.
       *[other] Se copiaron { $count } archivos de modelo. Pulsa «Descargar» para instalarlos.
    }
ai-sideload-none = No se encontraron archivos de modelo.
# $file is a file name, or several separated by commas.
ai-sideload-unknown = { $file } no es un modelo que { -app-name } conozca. No se copió.
ai-sideload-failed = No se pudo copiar { $file }: { $reason }
# Asked before using a file whose SHA-256 isn't in the shipped list. $model is the model it
# would be used as ("clip-vit-b32-1").
ai-unverified-title = ¿Usar un modelo sin verificar?
ai-unverified-detail = { $file } no está en la lista de modelos incluidos en { -app-name }, así que no se puede comprobar. Se usará como { $model }.
ai-unverified-use = Usar modelo sin verificar

# machine-drafted, needs native review
ai-failed-internal = Algo falló dentro de { -app-name }. El registro tiene los detalles.
# The buttons under a model-size question: switch, or keep the size in use.
ai-size-button = { $action ->
        [switch] Cambiar
       *[keep] Mantener el tamaño actual
    }
