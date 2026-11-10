# machine-drafted, needs native review
# Specular — Importar desde cámaras, teléfonos y tarjetas de memoria. Español.
# La tarjeta solo se lee; los archivos se borran de ella solo si se pide, tras una copia verificada.

palette-import = Importar fotos…
disabled-no-import-source = No hay ninguna tarjeta de memoria, cámara ni teléfono conectado

## Configuración → Importar
settings-section-import = Importar
settings-import-on-card = Al insertar una tarjeta
settings-import-on-card-help = Qué ocurre al conectar una tarjeta de memoria, una cámara o un teléfono con fotos.
settings-import-on-card-ask = Preguntar
settings-import-on-card-open = Abrir Importar
settings-import-on-card-nothing = No hacer nada
settings-import-destination = Importar en
settings-import-destination-help = La carpeta donde van las fotos y los vídeos importados. Sin definir, van a tu carpeta Imágenes.
settings-import-destination-unset = Carpeta Imágenes
settings-import-folder-pattern = Carpetas
settings-import-folder-pattern-help = Las carpetas que se crean dentro: {"{"}year{"}"}, {"{"}month{"}"}, {"{"}day{"}"}, {"{"}camera{"}"} y {"{"}event{"}"}, separadas por /.
settings-import-rename-pattern = Cambiar nombre
settings-import-rename-pattern-help = Nombres de archivo nuevos, como en Convertir: {"{"}name{"}"}, {"{"}n:03{"}"}, {"{"}date:%Y%m%d{"}"}, {"{"}camera{"}"}, {"{"}event{"}"}. Vacío conserva los nombres de la cámara.
settings-import-keep-pairs = Mantener juntos RAW y JPEG
settings-import-keep-pairs-help = Un archivo RAW y el JPEG tomado con él van a la misma carpeta con el mismo nombre, aunque se renombren.
settings-import-low-res-videos = Importar copias de vídeo de baja resolución
settings-import-low-res-videos-help = Algunas cámaras graban una copia pequeña de vista previa de cada vídeo (.LRV). Desactivado, se quedan en la tarjeta.
settings-import-eject = Expulsar al terminar
settings-import-eject-help = Expulsa la tarjeta cuando todo está copiado, para poder sacarla.

## El aviso al insertar una tarjeta
import-offer = ¿Importar desde { $name }?
import-offer-button = Importar

## La vista de importación
import-title = Importar
import-from = desde { $name }
import-source-next = Siguiente origen
import-close = Cerrar
import-scanning = Buscando fotos nuevas… { $done } de { $total }
import-scanning-start = Buscando fotos nuevas…
import-none-new = Todo lo que hay en { $name } ya se importó antes.
import-empty = No hay fotos ni vídeos en { $name }.
import-no-source = No hay ninguna tarjeta de memoria, cámara ni teléfono conectado.
import-summary = { $count ->
    [one] { $count } elemento nuevo
    [many] { $count } elementos nuevos
   *[other] { $count } elementos nuevos
} · { $size }
import-already = { $count } importados antes
import-unreadable = { $count ->
    [one] No se pudo leer { $count } archivo
    [many] No se pudieron leer { $count } archivos
   *[other] No se pudieron leer { $count } archivos
}
import-selected = { $count } de { $total } seleccionados
import-select-all = Seleccionar todo
import-select-none = No seleccionar nada
import-badge-pair = RAW+JPEG
import-badge-raw = RAW
import-badge-video = Vídeo

import-destination = Importar en
import-choose = Elegir…
import-folder-pattern = Carpetas
import-event = Evento
import-event-hint = Opcional, para {"{"}event{"}"}
import-rename = Cambiar nombre
import-rename-hint = Vacío conserva los nombres de la cámara
import-preview = Primer archivo: { $path }
import-pattern-error = { $error }
import-keep-pairs = Mantener juntos RAW y JPEG
import-eject = Expulsar al terminar
import-delete = Borrar de la tarjeta tras importar
import-free = { $size } por copiar · { $free } de espacio libre
import-free-unknown = { $size } por copiar
import-no-space = No hay suficiente espacio libre en { $folder }: esta importación necesita { $need } y solo hay { $free } libres. Libera espacio o elige otra carpeta.
import-start = { $count ->
    [one] Importar { $count } elemento
    [many] Importar { $count } elementos
   *[other] Importar { $count } elementos
}
import-nothing-selected = Selecciona las fotos que quieres importar

import-confirm-delete = { $count ->
    [one] Tras copiarlo y comprobarlo, se borrará { $count } archivo de { $name }. No se puede deshacer.
    [many] Tras copiarlos y comprobarlos, se borrarán { $count } archivos de { $name }. No se puede deshacer.
   *[other] Tras copiarlos y comprobarlos, se borrarán { $count } archivos de { $name }. No se puede deshacer.
}
import-confirm-yes = Importar y borrar
import-confirm-no = Volver

import-progress = Importando { $done } de { $total } · { $size }
import-progress-eta = Importando { $done } de { $total } · { $size } · { $time }
import-paused = En pausa · { $done } de { $total }
import-pause = Pausar
import-resume = Reanudar
import-cancel = Cancelar
import-hide = Ocultar
import-stopping = Deteniendo…
import-running = Ya hay una importación en curso.

import-done = { $count ->
    [one] { $count } archivo importado
    [many] { $count } archivos importados
   *[other] { $count } archivos importados
} en { $folder }
import-done-existed = { $count ->
        [one] { $count } ya estaba
       *[other] { $count } ya estaban
    }
import-done-deleted = { $count } borrados de la tarjeta
import-done-failed = { $count ->
    [one] No se pudo importar { $count } archivo
    [many] No se pudieron importar { $count } archivos
   *[other] No se pudieron importar { $count } archivos
}
import-done-cancelled = Importación cancelada tras { $count ->
        [one] { $count } archivo
       *[other] { $count } archivos
    }. Al importar de nuevo, continúa donde se detuvo.
import-done-nothing = No se importó nada nuevo.
import-ejected = Ya puedes sacar { $name }.
import-eject-failed = No se pudo expulsar { $name }: { $error }
import-failed-destination = No se puede usar la carpeta { $folder }: { $error }
import-ledger-failed = No se pudo leer la lista de archivos importados ({ $error }); puede que se vuelvan a ofrecer archivos ya importados.

## Lector de pantalla
import-a11y-group = Importar
import-a11y-tile = { $name }, { $date }
import-a11y-tile-selected = { $name }, { $date }, seleccionado
