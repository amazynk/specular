# Integración profunda con el escritorio: Ajustes → Integración, establecer como fondo.

settings-section-integration = Integración
settings-integration-default-viewer = Visor de imágenes predeterminado
settings-integration-default-viewer-help = Abrir las imágenes con { -app-name } al hacer doble clic. Al quitarlo se restaura el visor anterior.
# machine-drafted, needs native review
settings-integration-extra-default-types = Más tipos de archivo
settings-integration-extra-default-types-help = Otros tipos de archivo que abrir con { -app-name } al hacer doble clic; se eligen en Visor de imágenes predeterminado.
settings-integration-thumbnailer = Miniaturas en el gestor de archivos
settings-integration-thumbnailer-help = Permitir que los gestores de archivos muestren vistas previas de formatos que no saben leer, como JPEG XL, HEIC y archivos RAW de cámara.
settings-integration-single-instance = Instancia única mediante D-Bus
settings-integration-single-instance-help = Los archivos abiertos desde el gestor de archivos, un lanzador o la terminal van a la ventana de { -app-name } en ejecución. Se aplica a las ventanas abiertas después.
settings-integration-file-manager-actions = Acciones del gestor de archivos
settings-integration-file-manager-actions-help = Añadir «Abrir», «Comparar» y «Establecer como fondo» a los menús contextuales de Nautilus, Dolphin, Thunar y Nemo.
settings-integration-search-provider = Búsqueda del escritorio
# machine-drafted, needs native review
settings-integration-search-provider-help = Encontrar imágenes de tu biblioteca en la búsqueda de GNOME y KDE. Las palabras funcionan como en la barra de búsqueda de la biblioteca.
# machine-drafted, needs native review
settings-integration-url-scheme = Abrir enlaces specular://
settings-integration-url-scheme-help = Los enlaces como specular:///home/me/photo.jpg en un navegador u otro programa se abren en { -app-name }. Solo se abren archivos de este equipo, y los inusuales solo tras tu confirmación.

integration-state-on = Activado
integration-state-off = Desactivado
integration-state-partial = Configurado en parte. Vuelva a aplicarlo para repararlo.
integration-state-unavailable = No disponible en este escritorio
integration-state-sandboxed = No disponible dentro de un entorno aislado (Flatpak)
integration-state-no-bus = No disponible: no hay sesión de D-Bus
integration-state-no-file-manager = No disponible: no se encontró ningún gestor de archivos compatible
integration-state-no-search = No disponible: no se encontró GNOME Shell ni KRunner
integration-state-working = Trabajando…
integration-state-details = { $state } · { $details }
integration-detail-types = tipos de imagen: { $count } de { $total }
integration-detail-thumbnail-types = tipos de imagen: { $count }
integration-apply = Aplicar
integration-remove = Quitar
integration-done-on = «{ $option }» está activado
integration-done-off = «{ $option }» está desactivado
integration-failed = No se pudo cambiar «{ $option }»: { $reason }
# machine-drafted, needs native review
integration-search-remove-ask = ¿Desactivar la búsqueda del escritorio?
integration-search-remove-detail = { -app-name } se quita de la búsqueda de { $desktops }. Tu biblioteca y tus imágenes se quedan.
search-provider-open-folder = Abrir carpeta

integration-label-open = Abrir en { -app-name }
integration-label-compare = Comparar en { -app-name }
integration-label-wallpaper = Establecer como fondo
integration-label-generic-name = Visor de imágenes
integration-label-comment = Abra y recorra fotos grandes al instante

palette-set-wallpaper = Establecer como fondo
wallpaper-set = Fondo establecido ({ $method })
wallpaper-failed = No se pudo establecer el fondo: { $reason }

# machine-drafted, needs native review
url-invalid = No se puede abrir este enlace de { -app-name }: { $reason }
url-not-found = Los archivos que indica este enlace no existen
url-remote-refused = { -app-name } solo abre archivos de este equipo, no { $source }
url-confirm-files = { $more ->
    [0] «{ $file }»
   *[other] «{ $file }» y { $more } más
}
url-confirm-title = { $reason ->
    [dots] ¿Abrir { url-confirm-files }? El enlace llega a través de carpetas «..».
    [network] ¿Abrir { url-confirm-files }? Está en un recurso de red o un dispositivo.
   *[hidden] ¿Abrir { url-confirm-files }? Su nombre contiene caracteres invisibles o de dirección del texto.
}
url-confirm-open = Abrir

# Launcher actions and recent files: machine-drafted, needs native review
integration-label-new-window = Ventana nueva
integration-label-library = Biblioteca
integration-label-slideshow = Presentación de Imágenes
settings-integration-recent-files = Archivos recientes del escritorio
settings-integration-recent-files-help = Las imágenes que abra aparecen en «Recientes» del gestor de archivos y en las listas de archivos recientes del escritorio. Al quitarlo, se retiran de esas listas las entradas de { -app-name }.
integration-detail-recent = archivos: { $count }
# machine-drafted, needs native review
integration-extra-title = Más tipos de archivo
# machine-drafted, needs native review
integration-extra-intro = Además de imágenes, { -app-name } puede abrir estos archivos al hacer doble clic. No hay nada marcado hasta que tú lo marques. Los programas que editan estos archivos siguen en «Abrir con», y al desmarcar un tipo vuelve al programa que tenía antes. Blackmagic RAW, DWG, STEP, IGES y Zeiss CZI también necesitan software gratuito de sus fabricantes: ver Configuración › Decodificadores opcionales.
integration-extra-chosen = { $count ->
    [0] Ninguno elegido
    [one] 1 tipo elegido
    [many] { $count } de tipos elegidos
   *[other] { $count } tipos elegidos
}
integration-extra-count = { $on } de { $total }
integration-extra-none-yet = Ninguno todavía
integration-extra-keywords = tipos de archivo formatos asociaciones doble clic abrir con editor ciencia médico CAD SIG GIS 3D moléculas vídeo SVG PDF
integration-extra-group-editors = Archivos de editores
# machine-drafted, needs native review
integration-extra-group-editors-help = Documentos de programas de pintura y dibujo: Krita, GIMP, Paint.NET, PaintShop Pro, Procreate, Affinity, OpenRaster, Illustrator, CorelDRAW, Clip Studio Paint, Sketch y Figma. El doble clic muestra la imagen; se sigue editando en el programa que la creó.
integration-extra-group-science = Ciencia
integration-extra-group-science-help = Archivos de instrumentos y software de investigación: cámaras térmicas, microscopios, medidas de superficie, HDF5, NetCDF, datos sísmicos y meteorológicos.
integration-extra-group-medical = Medicina
integration-extra-group-medical-help = Exploraciones médicas e imagen cerebral: DICOM, NIfTI, MINC, NRRD, tractografía y superficies cerebrales.
integration-extra-group-cad-gis = CAD y SIG
# machine-drafted, needs native review
integration-extra-group-cad-gis-help = Dibujos de plóter, ilustraciones técnicas, diseños de chips y mapas: HP-GL, CGM, GDSII, shapefiles, GeoPackage, KML, GPX, GeoJSON y teselas de elevación. DWG se abre con ODA File Converter, gratuito, y STEP e IGES con OpenCASCADE.
integration-extra-group-models = 3D y moléculas
integration-extra-group-models-help = Modelos 3D, modelos de edificios, nubes de puntos y moléculas: STL, OBJ, glTF, PLY, 3MF, X3D, IFC, LAS, E57, PDB, CIF y más.
integration-extra-group-videos = Vídeos
# machine-drafted, needs native review
integration-extra-group-videos-help = Vídeos: MP4, MOV, M4V, 3GP, 3G2, MKV, WebM, AVCHD (MTS, M2TS), OGV, IVF y MXF. Blackmagic RAW se abre con el Blackmagic RAW SDK, gratuito.
integration-extra-group-other-images = SVG y otras imágenes
# machine-drafted, needs native review
integration-extra-group-other-images-help = Imágenes y documentos que suelen abrir otros programas: dibujos SVG, PDF, XPS, DjVu, EPS, WMF y EMF, Visio, OpenDocument, WordPerfect Graphics, libros EPUB y fuentes, y animaciones como MNG, FLIC y cursores animados.
integration-extra-group-text = Texto, tablas y subtítulos
integration-extra-group-text-help = Texto plano y registros, NFO, Markdown, tablas CSV y TSV, RTF y archivos de subtítulos: SRT, WebVTT, ASS, SSA, MicroDVD, PGS, LRC, TTML y SCC.
