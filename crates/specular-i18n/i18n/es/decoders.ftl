# machine-drafted, needs native review

settings-section-decoders = Decodificadores opcionales
settings-decoders-look-again = Buscar de nuevo
settings-decoders-look-again-help = Busca decodificadores instalados desde entonces.
    { -app-name } también busca cada vez que su ventana pasa al frente.
settings-decoders-braw = Usar el Blackmagic RAW SDK
settings-decoders-braw-help = Los clips Blackmagic RAW se abren con él cuando está instalado.
settings-decoders-braw-path = Blackmagic RAW SDK
settings-decoders-braw-path-help = Déjalo vacío para buscarlo donde lo pone su instalador.
settings-decoders-dwg = Usar ODA File Converter
settings-decoders-dwg-help = Los dibujos DWG se abren con él cuando está instalado.
    Una copia del dibujo se convierte en una carpeta privada, que luego se borra.
settings-decoders-dwg-path = Programa ODA File Converter
settings-decoders-dwg-path-help = Déjalo vacío para buscarlo donde lo pone su instalador.
settings-decoders-occt = Usar OpenCASCADE
settings-decoders-occt-help = Los modelos STEP e IGES se abren con él cuando está instalado.
settings-decoders-occt-path = Bibliotecas de OpenCASCADE
settings-decoders-occt-path-help = Déjalo vacío para buscarlas donde el sistema guarda las bibliotecas.
# machine-drafted, needs native review
settings-decoders-czi = Usar libCZI
settings-decoders-czi-help = Los archivos Zeiss CZI se abren con él cuando está instalado.
    Se ejecuta en el asistente aislado de { -app-name }, sin acceso a tus otros archivos.
settings-decoders-czi-path = Carpeta de la biblioteca libCZI
settings-decoders-czi-path-help = Déjalo vacío para buscarla donde el sistema guarda las bibliotecas.

decoders-intro = Algunos formatos se abren con software gratuito de sus fabricantes.
    Instálalo una vez: { -app-name } lo encuentra y lo usa. Nunca lo descarga.
decoders-keywords = decodificadores opcionales fabricante SDK códec complemento Blackmagic RAW BRAW RED R3D DWG ODA File Converter OpenCASCADE STEP IGES Zeiss CZI libCZI necesita decodificador
decoders-row = { $format } ({ $extensions })
decoders-state-found = Encontrado
decoders-state-off = Desactivado
decoders-state-not-found = No instalado
decoders-state-broken = No se puede usar
decoders-state-refused = No es posible
decoders-state-planned = Aún no disponible
decoders-state-checking = Buscando…
decoders-uses = Se abre con { $decoder } de { $vendor }.
decoders-not-found = Es gratuito. Instálalo y pulsa Buscar de nuevo.
decoders-off = Desactivado: estos archivos no se abren.
decoders-version = Versión { $version }.
decoders-r3d-why = La licencia de RED solo deja leer archivos R3D a programas hechos con su SDK, según los términos de RED.
    { -app-name } no puede aceptar esos términos, así que los archivos R3D no se abren.
decoders-ecw-why = La licencia de Hexagon solo permite usar su ECW SDK a los programas que lo incluyen, no a una copia que hayas instalado tú.
    Por eso los archivos ECW no se abren. Los JPEG 2000 creados con el ECW SDK sí.
decoders-mrsid-why = La licencia del MrSID SDK solo permite usarlo a los programas que lo incluyen, no a una copia que hayas instalado tú.
    Por eso los archivos MrSID no se abren.
# machine-drafted, needs native review
# DJI radiometric JPEGs (drone thermal cameras): the photo and metadata open; the
# temperatures would need DJI's SDK.
decoders-dji-thermal-why = La licencia del SDK de DJI solo permite usarlo a los programas que lo incluyen, no a una copia que hayas instalado tú. La foto y sus metadatos se abren igualmente.
decoders-occt-why = Los sólidos STEP e IGES necesitan OpenCASCADE, que esta versión de { -app-name } no puede usar.
decoders-get-row = La página de descarga del fabricante
decoders-get = Descargar
decoders-locate-row = ¿Instalado en otro sitio?
decoders-locate = Buscar…
decoders-forget-row = Lo elegiste a mano.
decoders-forget = Olvidar…
decoders-forget-ask = ¿Olvidar el { $decoder } que elegiste?
decoders-forget-detail = { -app-name } vuelve a buscarlo por sí mismo. No se borra nada del disco.
decoders-forget-ok = Olvidar
decoders-locate-title = Buscar { $decoder }
decoders-locate-wrong = Eso no es { $decoder }.
decoders-located = Usando el { $decoder } que elegiste.
decoders-forgotten = { -app-name } vuelve a buscar { $decoder } por sí mismo.
decoders-still-missing = { $decoder } sigue sin encontrarse.
decoders-now-found = { $decoder } encontrado.
decoders-looked = Decodificadores buscados de nuevo.

decoders-card-title = { $format } necesita el decodificador gratuito de { $vendor }
decoders-card-what = { $file } se abre con { $decoder }. Es gratuito.
decoders-card-how = Instálalo y vuelve: { -app-name } lo encuentra por sí mismo.
decoders-card-get = Descargar
decoders-card-installed = Ya lo instalé
decoders-card-locate = Buscar…

gallery-badge-needs-decoder = Necesita decodificador
gallery-needs-decoder = Se abre con { $decoder } de { $vendor }, gratuito pero no instalado.

error-reason-needs-decoder = Los archivos { $format } necesitan { $decoder } de { $vendor }, gratuito pero no instalado.
