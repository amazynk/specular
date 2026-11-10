# machine-drafted, needs native review

settings-section-decoders = Decodificadores opcionais
settings-decoders-look-again = Procurar de novo
settings-decoders-look-again-help = Procura decodificadores instalados desde então.
    O { -app-name } também procura sempre que sua janela vem para a frente.
settings-decoders-braw = Usar o Blackmagic RAW SDK
settings-decoders-braw-help = Clipes Blackmagic RAW abrem com ele quando está instalado.
settings-decoders-braw-path = Blackmagic RAW SDK
settings-decoders-braw-path-help = Deixe vazio para encontrá-lo onde o instalador o coloca.
settings-decoders-dwg = Usar o ODA File Converter
settings-decoders-dwg-help = Desenhos DWG abrem com ele quando está instalado.
    Uma cópia do desenho é convertida em uma pasta privada, que depois é apagada.
settings-decoders-dwg-path = Programa ODA File Converter
settings-decoders-dwg-path-help = Deixe vazio para encontrá-lo onde o instalador o coloca.
settings-decoders-occt = Usar o OpenCASCADE
settings-decoders-occt-help = Modelos STEP e IGES abrem com ele quando está instalado.
settings-decoders-occt-path = Bibliotecas do OpenCASCADE
settings-decoders-occt-path-help = Deixe vazio para encontrá-las onde o sistema guarda as bibliotecas.
# machine-drafted, needs native review
settings-decoders-czi = Usar libCZI
settings-decoders-czi-help = Arquivos Zeiss CZI abrem com ele quando está instalado.
    Ele roda no auxiliar isolado do { -app-name }, sem acesso aos seus outros arquivos.
settings-decoders-czi-path = Pasta da biblioteca libCZI
settings-decoders-czi-path-help = Deixe vazio para encontrá-la onde o sistema guarda as bibliotecas.

decoders-intro = Alguns formatos abrem com software gratuito dos seus fabricantes.
    Instale uma vez: o { -app-name } o encontra e o usa. Ele nunca o baixa.
decoders-keywords = decodificadores opcionais fabricante SDK codec plugin Blackmagic RAW BRAW RED R3D DWG ODA File Converter OpenCASCADE STEP IGES Zeiss CZI libCZI precisa de decodificador
decoders-row = { $format } ({ $extensions })
decoders-state-found = Encontrado
decoders-state-off = Desligado
decoders-state-not-found = Não instalado
decoders-state-broken = Não pode ser usado
decoders-state-refused = Não é possível
decoders-state-planned = Ainda não disponível
decoders-state-checking = Procurando…
decoders-uses = Abre com o { $decoder } de { $vendor }.
decoders-not-found = É gratuito. Instale e depois clique em Procurar de novo.
decoders-off = Desligado: estes arquivos não abrem.
decoders-version = Versão { $version }.
decoders-r3d-why = A licença da RED só permite ler arquivos R3D a programas feitos com o SDK dela, nos termos da própria RED.
    O { -app-name } não pode aceitar esses termos, então arquivos R3D não abrem.
decoders-ecw-why = A licença da Hexagon só permite usar o ECW SDK em programas que o incluem, não em uma cópia que você instalou.
    Por isso, arquivos ECW não abrem. Arquivos JPEG 2000 criados com o ECW SDK abrem.
decoders-mrsid-why = A licença do MrSID SDK só permite usá-lo em programas que o incluem, não em uma cópia que você instalou.
    Por isso, arquivos MrSID não abrem.
# machine-drafted, needs native review
# DJI radiometric JPEGs (drone thermal cameras): the photo and metadata open; the
# temperatures would need DJI's SDK.
decoders-dji-thermal-why = A licença do SDK da DJI só permite usá-lo em programas que o incluem, não em uma cópia que você instalou. A foto e seus metadados abrem mesmo assim.
decoders-occt-why = Sólidos STEP e IGES precisam do OpenCASCADE, que esta versão do { -app-name } não pode usar.
decoders-get-row = A página de download do fabricante
decoders-get = Baixar
decoders-locate-row = Instalado em outro lugar?
decoders-locate = Localizar…
decoders-forget-row = Você escolheu este manualmente.
decoders-forget = Esquecer…
decoders-forget-ask = Esquecer o { $decoder } que você escolheu?
decoders-forget-detail = O { -app-name } volta a procurá-lo sozinho. Nada é removido do disco.
decoders-forget-ok = Esquecer
decoders-locate-title = Localizar o { $decoder }
decoders-locate-wrong = Isso não é o { $decoder }.
decoders-located = Usando o { $decoder } que você escolheu.
decoders-forgotten = O { -app-name } volta a procurar o { $decoder } sozinho.
decoders-still-missing = O { $decoder } ainda não foi encontrado.
decoders-now-found = { $decoder } encontrado.
decoders-looked = Decodificadores procurados de novo.

decoders-card-title = { $format } precisa do decodificador gratuito de { $vendor }
decoders-card-what = { $file } abre com o { $decoder }. É gratuito.
decoders-card-how = Instale e depois volte: o { -app-name } o encontra sozinho.
decoders-card-get = Baixar
decoders-card-installed = Já instalei
decoders-card-locate = Localizar…

gallery-badge-needs-decoder = Precisa de decodificador
gallery-needs-decoder = Abre com o { $decoder } de { $vendor }, gratuito, mas não instalado.

error-reason-needs-decoder = Arquivos { $format } precisam do { $decoder } de { $vendor }, gratuito, mas não instalado.
