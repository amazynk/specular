# Specular — tethered capture: a camera on USB, its shots downloaded as they are taken. Español. machine-drafted, needs native review

palette-tether = Captura conectada…
palette-tether-capture = Tomar una foto (conectada)

tether-title = Captura conectada
tether-close = Cerrar la captura conectada
tether-default-folder = Captura conectada

## Cameras
tether-none = No se encontró ninguna cámara.
tether-none-hint = Conecta la cámara con su cable USB y enciéndela. Si tiene modo USB, elige PTP o conexión a PC.
tether-look-again = Buscar de nuevo
tether-connect = Conectar
tether-next-camera = Siguiente cámara
tether-connecting = Conectando…
tether-connected = { $camera } conectada
tether-disconnect = Desconectar
tether-try-again = Reintentar
tether-gone = La cámara se desconectó.
tether-not-connected = No hay ninguna cámara conectada. Abre primero la captura conectada.

## Why a camera can't be opened
tether-permission = Este usuario no puede abrir el dispositivo USB de la cámara { $node }.
tether-permission-rule = Una regla de udev lo permite. Como administrador, pon esta línea en { $file }:
tether-permission-reload = Después ejecuta «sudo udevadm control --reload» y vuelve a conectar la cámara.
tether-copy-rule = Copiar regla
tether-busy-other = Otro programa está usando la cámara.
tether-busy-named = La cámara está en uso por { $programs }.
tether-busy-hint = Cierra ese programa o desmonta la cámara en el gestor de archivos y vuelve a intentarlo.
tether-driver = El controlador del sistema para esta cámara no permite que otros programas la usen.

## The camera
tether-battery = Batería { PERCENT($percent) }
tether-card = { $card }: { $free } libres
tether-card-shots = { $card }: { $free } libres, { $shots } fotos
tether-no-card = No hay tarjeta de memoria.
tether-no-settings = La cámara no informa de sus ajustes de exposición.
tether-no-remote = Esta cámara no se puede disparar desde aquí. Se descargan las fotos tomadas con su disparador.
tether-capture = Disparar
tether-step-down = { $setting }: bajar ({ $value })
tether-step-up = { $setting }: subir ({ $value })

## Exposure settings
tether-prop-mode = Modo
tether-prop-shutter = Obturación
tether-prop-aperture = Diafragma
tether-prop-iso = ISO
tether-prop-bias = Compensación de exposición
tether-prop-white-balance = Balance de blancos
tether-auto = Automático
tether-bulb = Bulb (B)
tether-camera-value = Ajuste de la cámara { $value }
tether-wb-manual = Manual
tether-wb-one-push = Medición puntual
tether-wb-daylight = Luz de día
tether-wb-fluorescent = Fluorescente
tether-wb-tungsten = Tungsteno
tether-wb-flash = Flash
tether-mode-manual = Manual
tether-mode-program = Programa
tether-mode-aperture = Prioridad a la apertura
tether-mode-shutter = Prioridad a la velocidad
tether-mode-creative = Creativo
tether-mode-action = Deportes
tether-mode-portrait = Retrato

## Where shots go
tether-folder = Carpeta
tether-names = Nombres
tether-names-camera = Los nombres de la cámara
tether-arrival = Estilo de Develop
tether-choose-folder = Carpeta…
tether-choose-style = Estilo…
tether-downloading = Descargando… { PERCENT($percent) }
tether-saved = { $name } guardada
tether-saved-count = { $count ->
    [one] { $count } foto guardada
   *[other] { $count } fotos guardadas
}

## Failures
tether-busy = La cámara está ocupada. Vuelve a intentarlo.
tether-card-full = La tarjeta de memoria está llena.
tether-set-refused = La cámara no aceptó ese ajuste. Revisa el dial de modos.
tether-disk = No se pudo guardar la foto: { $error }
tether-error = Error de la cámara: { $error }

## Settings › Cull mode
settings-cull-tether-folder = Carpeta de la captura conectada
settings-cull-tether-folder-help = Dónde se guardan las fotos de una cámara conectada por cable.
settings-cull-tether-folder-unset = Imágenes › Captura conectada
settings-cull-tether-rename = Nombres de archivo de la captura conectada
settings-cull-tether-rename-help = Nombres nuevos, con las mismas etiquetas que Renombrar en Importar. Vacío conserva los nombres de la cámara.
