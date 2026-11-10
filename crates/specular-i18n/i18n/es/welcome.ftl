# machine-drafted, needs native review
# Specular — La pantalla de inicio (al abrir Specular sin un archivo y al cerrar la última
# imagen), sus tarjetas «Para probar» y las sugerencias que se muestran una sola vez. Español.

## Pantalla de inicio

welcome-title = { -app-name }
welcome-tagline = Un visor rápido y fiel para fotos y para imágenes técnicas, científicas y médicas.
welcome-open = Abrir un archivo…
welcome-open-about = Una imagen, un documento o un archivo de datos.
welcome-browse = Explorar una carpeta…
welcome-browse-about = Elige una carpeta y mírala como galería.
welcome-pictures = Tus imágenes
welcome-pictures-about = Todas las fotos de tu biblioteca, por fecha y lugar.
welcome-pictures-setup-title = Configurar tu biblioteca…
welcome-pictures-setup = A partir de tu carpeta { $folder }.
welcome-pictures-ask = ¿Añadir tu carpeta { $folder } a la biblioteca?
welcome-pictures-add = Añadir carpeta
welcome-drop = También puedes soltar archivos o carpetas en cualquier lugar de esta ventana.
welcome-recent = Carpetas recientes
welcome-recent-name = { $name }, en { $parent }
welcome-tips = Para probar
welcome-tips-hide = Ocultar estas sugerencias
welcome-tips-hidden = Sugerencias ocultas. «Mostrar bienvenida» en la paleta de comandos las recupera.

## Para probar: un nombre y una línea cada una. La tecla junto al nombre es la del usuario.

welcome-tip-gallery = Galería
welcome-tip-gallery-about = Ver una carpeta entera de un vistazo.
welcome-tip-compare = Comparar
welcome-tip-compare-about = Selecciona de dos a cuatro imágenes en la galería para verlas una junto a otra.
welcome-tip-cull = Modo de criba
welcome-tip-cull-about = Elegir, descartar y valorar una sesión con una tecla para cada cosa.
welcome-tip-slideshow = Presentación
welcome-tip-slideshow-about = Mostrar una carpeta a pantalla completa, una imagen tras otra.
welcome-tip-palette = Paleta de comandos
welcome-tip-palette-about = Buscar cualquier comando o ajuste por su nombre.
welcome-tip-studies = Estudios
welcome-tip-studies-about = Los pacientes, estudios y series de un disco o carpeta DICOM.
welcome-tip-dicom-servers = Servidores DICOMweb
welcome-tip-dicom-servers-about = Explorar y abrir estudios de un PACS.
welcome-tip-dicom-tags = Etiquetas DICOM
welcome-tip-dicom-tags-about = Todas las etiquetas de una imagen, con búsqueda.
welcome-tip-window-level = Ventana y nivel
welcome-tip-window-level-about = Arrastra para ajustar el contraste de una imagen DICOM.
welcome-tip-omero-servers = Servidores OMERO
welcome-tip-omero-servers-about = Explorar proyectos de microscopía en OMERO e IDR.
welcome-tip-stretch = Estiramiento
welcome-tip-stretch-about = Resaltar detalles tenues en datos astronómicos y científicos.
welcome-tip-drawing-layers = Capas del dibujo
welcome-tip-drawing-layers-about = Mostrar y ocultar las capas de un dibujo técnico.
welcome-tip-needs-image = Abre primero una imagen y luego pulsa { $key }.
welcome-tip-needs-gallery = En la galería, selecciona de dos a cuatro imágenes y luego pulsa { $key }.
welcome-tip-key = Atajo: { $key }

## Sugerencias únicas: una vez cada una, discretas, como mucho una por inicio. Configuración ›
## General › Mostrar sugerencias las desactiva todas.

hint-gallery = Pulsa { $key } para ver esta carpeta como galería.
hint-gallery-first = Selecciona de dos a cuatro imágenes y pulsa { $compare } para compararlas, o { $cull } para cribar la carpeta.
hint-raw = Una carpeta de fotos RAW: pulsa { $key } para cribarla (elegir, descartar y valorar).
hint-pages =
    { $count ->
        [one] Este archivo tiene { $count } página. { $next } y { $prev } pasan de página.
        [many] Este archivo tiene { $count } páginas. { $next } y { $prev } pasan de página.
       *[other] Este archivo tiene { $count } páginas. { $next } y { $prev } pasan de página.
    }
hint-turn-off = Desactivar sugerencias
hint-turned-off = Sugerencias desactivadas. Configuración › General › Mostrar sugerencias las vuelve a activar.

## Paleta de comandos

palette-show-welcome = Mostrar bienvenida
palette-show-welcome-help = La pantalla de inicio, con Abrir, Explorar, tus imágenes y cosas para probar.
palette-close = Cerrar
palette-close-help = Cerrar la imagen o la carpeta y volver a la pantalla de inicio. En la pantalla de inicio, salir.

## Primer inicio (bajo la pregunta del perfil)

profile-on-start = Cuando { -app-name } se abre sin un archivo:
profile-skip = Omitir
