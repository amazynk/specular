# machine-drafted, needs native review

settings-section-decoders = Descodificadores opcionais
settings-decoders-look-again = Procurar de novo
settings-decoders-look-again-help = Procura descodificadores instalados entretanto.
    O { -app-name } também procura sempre que a sua janela passa para a frente.
settings-decoders-braw = Usar o Blackmagic RAW SDK
settings-decoders-braw-help = Os clipes Blackmagic RAW abrem com ele quando está instalado.
settings-decoders-braw-path = Blackmagic RAW SDK
settings-decoders-braw-path-help = Deixe vazio para o encontrar onde o instalador o coloca.
settings-decoders-dwg = Usar o ODA File Converter
settings-decoders-dwg-help = Os desenhos DWG abrem com ele quando está instalado.
    Uma cópia do desenho é convertida numa pasta privada, que depois é eliminada.
settings-decoders-dwg-path = Programa ODA File Converter
settings-decoders-dwg-path-help = Deixe vazio para o encontrar onde o instalador o coloca.
settings-decoders-occt = Usar o OpenCASCADE
settings-decoders-occt-help = Os modelos STEP e IGES abrem com ele quando está instalado.
settings-decoders-occt-path = Bibliotecas do OpenCASCADE
settings-decoders-occt-path-help = Deixe vazio para as encontrar onde o sistema guarda as bibliotecas.
# machine-drafted, needs native review
settings-decoders-czi = Usar libCZI
settings-decoders-czi-help = Os ficheiros Zeiss CZI abrem com ele quando está instalado.
    É executado no auxiliar isolado do { -app-name }, sem acesso aos seus outros ficheiros.
settings-decoders-czi-path = Pasta da biblioteca libCZI
settings-decoders-czi-path-help = Deixe vazio para a encontrar onde o sistema guarda as bibliotecas.

decoders-intro = Alguns formatos abrem com software gratuito dos seus fabricantes.
    Instale-o uma vez: o { -app-name } encontra-o e usa-o. Nunca o descarrega.
decoders-keywords = descodificadores opcionais fabricante SDK codec plugin Blackmagic RAW BRAW RED R3D DWG ODA File Converter OpenCASCADE STEP IGES Zeiss CZI libCZI precisa de descodificador
decoders-row = { $format } ({ $extensions })
decoders-state-found = Encontrado
decoders-state-off = Desligado
decoders-state-not-found = Não instalado
decoders-state-broken = Não pode ser usado
decoders-state-refused = Não é possível
decoders-state-planned = Ainda não disponível
decoders-state-checking = A procurar…
decoders-uses = Abre com o { $decoder } de { $vendor }.
decoders-not-found = É gratuito. Instale-o e depois prima Procurar de novo.
decoders-off = Desligado: estes ficheiros não abrem.
decoders-version = Versão { $version }.
decoders-r3d-why = A licença da RED só deixa ler ficheiros R3D a programas feitos com o SDK dela, nos termos da própria RED.
    O { -app-name } não pode aceitar esses termos, por isso os ficheiros R3D não abrem.
decoders-ecw-why = A licença da Hexagon só permite utilizar o ECW SDK em programas que o incluem, não numa cópia que instalou.
    Por isso, os ficheiros ECW não abrem. Os ficheiros JPEG 2000 criados com o ECW SDK abrem.
decoders-mrsid-why = A licença do MrSID SDK só permite utilizá-lo em programas que o incluem, não numa cópia que instalou.
    Por isso, os ficheiros MrSID não abrem.
# machine-drafted, needs native review
# DJI radiometric JPEGs (drone thermal cameras): the photo and metadata open; the
# temperatures would need DJI's SDK.
decoders-dji-thermal-why = A licença do SDK da DJI só permite utilizá-lo em programas que o incluem, não numa cópia que instalou. A fotografia e os seus metadados abrem na mesma.
decoders-occt-why = Os sólidos STEP e IGES precisam do OpenCASCADE, que esta versão do { -app-name } não pode usar.
decoders-get-row = A página de transferência do fabricante
decoders-get = Transferir
decoders-locate-row = Instalado noutro sítio?
decoders-locate = Localizar…
decoders-forget-row = Escolheu este manualmente.
decoders-forget = Esquecer…
decoders-forget-ask = Esquecer o { $decoder } que escolheu?
decoders-forget-detail = O { -app-name } volta a procurá-lo sozinho. Nada é removido do disco.
decoders-forget-ok = Esquecer
decoders-locate-title = Localizar o { $decoder }
decoders-locate-wrong = Isso não é o { $decoder }.
decoders-located = A usar o { $decoder } que escolheu.
decoders-forgotten = O { -app-name } volta a procurar o { $decoder } sozinho.
decoders-still-missing = O { $decoder } continua sem ser encontrado.
decoders-now-found = { $decoder } encontrado.
decoders-looked = Descodificadores procurados de novo.

decoders-card-title = { $format } precisa do descodificador gratuito de { $vendor }
decoders-card-what = { $file } abre com o { $decoder }. É gratuito.
decoders-card-how = Instale-o e depois volte: o { -app-name } encontra-o sozinho.
decoders-card-get = Transferir
decoders-card-installed = Já o instalei
decoders-card-locate = Localizar…

gallery-badge-needs-decoder = Precisa de descodificador
gallery-needs-decoder = Abre com o { $decoder } de { $vendor }, gratuito, mas não instalado.

error-reason-needs-decoder = Os ficheiros { $format } precisam do { $decoder } de { $vendor }, gratuito, mas não instalado.
