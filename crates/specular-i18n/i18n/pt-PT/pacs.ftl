# machine-drafted, needs native review

## Send to PACS (C-STORE, STOW-RS), DIMSE servers, teaching quiz and teaching case export.

## Servers: DIMSE fields and Verify.

dicomweb-field-kind = Tipo
dicomweb-kind-web = DICOMweb
dicomweb-kind-dimse = DICOM (DIMSE)
dicomweb-field-host = Anfitrião
dicomweb-hint-host = pacs.example.org
dicomweb-field-port = Porta
dicomweb-hint-port = 104 ou 11112
# The application entity title the server answers to.
dicomweb-field-ae = AE title do servidor
dicomweb-hint-ae = PACS
# The application entity title Specular sends as.
dicomweb-field-our-ae = O nosso AE title
# machine-drafted, needs native review
dicomweb-field-tls = Ligação segura (TLS)
dicomweb-tls-on = Ligação segura ativada
dicomweb-tls-off = Ligação segura desativada
dicomweb-problem-host = Introduza o nome do anfitrião ou o endereço do servidor.
dicomweb-problem-port = A porta é um número de 1 a 65535.
dicomweb-problem-ae = Um AE title tem de 1 a 16 caracteres, sem barra invertida.
# machine-drafted, needs native review
dicomweb-dimse-note-open = Os servidores DICOM (DIMSE) podem ser pesquisados, abertos e receber envios.
# How series are fetched: C-GET (over our own connection) or C-MOVE (the server connects back).
# machine-drafted, needs native review
pacs-field-retrieve = Obter com
# machine-drafted, needs native review
pacs-retrieve-get = C-GET
# machine-drafted, needs native review
pacs-retrieve-move = C-MOVE
# C-MOVE: the address of this computer and the port the server sends to.
# machine-drafted, needs native review
pacs-field-receive-address = Receber no endereço
# machine-drafted, needs native review
pacs-hint-receive-address = Endereço deste computador, por ex. 192.168.1.20
# machine-drafted, needs native review
pacs-field-receive-port = Receber na porta
# machine-drafted, needs native review
pacs-hint-receive-port = Por ex. 11113
# machine-drafted, needs native review
pacs-move-note = Para C-MOVE, o servidor tem de conhecer o nosso AE title { $ae } com este endereço e esta porta.
    O Specular só fica à escuta enquanto chega uma série.
# machine-drafted, needs native review
pacs-problem-receive-address = Introduza o endereço deste computador (por ex. 192.168.1.20) onde receber.
# machine-drafted, needs native review
pacs-problem-receive-port = Introduza uma porta onde receber. O C-MOVE precisa de uma.
dicomweb-verify = Verificar
dicomweb-verifying = A verificar o servidor…
dicomweb-verify-ok = { $server } respondeu.

## The Studies browser and the palette.

studies-send = Enviar para o PACS…
studies-send-description = Envia esta série, o respetivo estudo ou tudo o que está no disco para um servidor.
palette-send-to-pacs = Enviar para o PACS…
palette-send-to-pacs-help = Enviar a série ou o estudo atual para um servidor PACS.

## The Send page.

pacs-title = Enviar para o PACS
# Under the title: what is being sent.
pacs-summary = { $studies ->
    [one] { $studies } estudo
    [many] { $studies } estudos
   *[other] { $studies } estudos
} · { $files ->
    [one] { $files } ficheiro
    [many] { $files } ficheiros
   *[other] { $files } ficheiros
}
pacs-what = Enviar
pacs-scope-series = Esta série ({ $files ->
    [one] { $files } ficheiro
    [many] { $files } ficheiros
   *[other] { $files } ficheiros
})
pacs-scope-study = Este estudo ({ $files ->
    [one] { $files } ficheiro
    [many] { $files } ficheiros
   *[other] { $files } ficheiros
})
pacs-scope-all = Tudo o que foi lido ({ $studies ->
    [one] { $studies } estudo
    [many] { $studies } estudos
   *[other] { $studies } estudos
}, { $files ->
    [one] { $files } ficheiro
    [many] { $files } ficheiros
   *[other] { $files } ficheiros
})
pacs-anonymise = Anonimizar
pacs-anonymise-note = Os dados do paciente são removidos das cópias enviadas. Os seus ficheiros não são alterados.
pacs-identified-note = Os ficheiros são enviados tal como estão, com os dados do paciente.
# The list of servers to choose from.
pacs-to = Para
pacs-no-servers = Ainda não há servidores. Adicione um em Servidores DICOMweb.
pacs-send = Enviar…
pacs-go-back = Voltar
pacs-confirm-send = Enviar
pacs-confirm = Enviar { $studies ->
    [one] { $studies } estudo
    [many] { $studies } estudos
   *[other] { $studies } estudos
} ({ $files ->
    [one] { $files } ficheiro
    [many] { $files } ficheiros
   *[other] { $files } ficheiros
}) para { $server }?
pacs-busy = Ainda está um envio em curso.
pacs-started = A enviar.
pacs-stopping = A parar…
pacs-sending = A enviar { $done } de { $total } para { $server }…
pacs-sent-all = { $total ->
    [one] { $total } ficheiro foi enviado para { $server }.
    [many] Todos os { $total } ficheiros foram enviados para { $server }.
   *[other] Todos os { $total } ficheiros foram enviados para { $server }.
}
pacs-sent-some = { $sent } de { $total } ficheiros foram enviados para { $server }.
pacs-stopped = Parado após { $sent } de { $total ->
        [one] { $total } ficheiro
       *[other] { $total } ficheiros
    }.
pacs-stopped-because = Parado após { $sent } de { $total ->
        [one] { $total } ficheiro
       *[other] { $total } ficheiros
    }: { $reason }
pacs-warnings = { $count ->
    [one] { $count } ficheiro foi armazenado com um aviso.
    [many] { $count } ficheiros foram armazenados com um aviso.
   *[other] { $count } ficheiros foram armazenados com um aviso.
}
# A file that was not sent: its file name and why.
pacs-failed-line = { $name }: { $reason }
pacs-failed-more = E mais { $count }.

## Why one file was not sent.

pacs-reason-unreadable = Não foi possível ler: { $reason }
pacs-reason-not-dicom = Não é um ficheiro DICOM
pacs-reason-burned-in = Tem dados do paciente gravados na imagem, por isso não é enviado anonimizado
pacs-reason-document = Um documento que não pode ser anonimizado
pacs-reason-anonymise = Não foi possível anonimizar: { $reason }
pacs-reason-not-accepted = O servidor não aceita { $class }
pacs-reason-convert = Não foi possível converter para o servidor: { $reason }
pacs-reason-resources = O servidor está sem espaço ou recursos
pacs-reason-mismatch = O servidor diz que os dados não correspondem ao respetivo tipo
pacs-reason-understand = O servidor não conseguiu lê-lo
pacs-reason-class = O servidor não suporta este tipo de objeto
pacs-reason-not-authorised = O servidor não o permite
pacs-reason-status = O servidor recusou-o (estado { $code })
pacs-reason-refused = O servidor recusou-o
pacs-reason-too-large = Demasiado grande para o servidor
pacs-reason-http = O servidor recusou-o (erro { $status })

## Why sending stopped, or Verify failed.

pacs-fatal-network-off = A rede está desligada.
pacs-fatal-unknown-host = { $host } não é um nome de anfitrião conhecido.
pacs-fatal-connect = Nada responde em { $host }, porta { $port }.
pacs-fatal-unreachable = Não é possível aceder a { $host }.
pacs-fatal-timeout = { $host } não respondeu a tempo.
pacs-fatal-cancelled = Parado.
pacs-fatal-called-ae = { $host } não responde ao AE title { $ae }. Verifique o AE title do servidor.
pacs-fatal-calling-ae = { $host } não conhece o nosso AE title { $ae }. Adicione-o no servidor.
pacs-fatal-busy = { $host } está ocupado. Tente novamente mais tarde.
pacs-fatal-rejected = { $host } recusou a ligação.
pacs-fatal-aborted = { $host } terminou a ligação.
pacs-fatal-nothing-accepted = { $host } não aceita nenhum destes tipos de objeto.
pacs-fatal-protocol = { $host } respondeu de uma forma que o Specular não compreende.
pacs-fatal-auth = { $host } não aceitou o início de sessão.
pacs-fatal-http = { $host } respondeu com o erro { $status }.

## Teaching quiz.

palette-teaching-quiz = Quiz de ensino
palette-teaching-quiz-help = Percorrer as imagens-chave ou um caso de ensino, com os achados ocultos até os revelar.
# "Case 2 of 5".
quiz-case = Caso { $n } de { $total }
quiz-hint-hidden = Espaço mostra os achados · → caso seguinte · Esc termina
quiz-hint-revealed = Espaço volta a ocultá-los · → caso seguinte · Esc termina
quiz-hidden = Achados ocultos.
quiz-no-findings = Nenhum achado registado.
quiz-notes = Notas do caso: { $notes }
quiz-ended = Quiz terminado.
quiz-empty = Não há casos para mostrar. Marque imagens-chave com K ou escolha uma pasta de caso de ensino.
quiz-not-a-case = Esta pasta não é um caso de ensino exportado pelo Specular.
quiz-pick-title = Escolher uma pasta de caso de ensino

## Teaching case folder (Export key images).

figure-keys-case = Pasta de caso de ensino
figure-case-notes = Notas do caso
figure-case-findings = Achados
figure-case-none = Nenhum registado
figure-case-readme = Caso de ensino exportado pelo Specular. Os dados do paciente foram removidos.
    images: as imagens-chave, com o texto gravado tapado a preto.
    DICOM: cópias anonimizadas e um manifesto Key Object Selection "For Teaching".
    case.txt: as notas do caso e os achados de cada imagem. case.json: o mesmo para scripts.
    Para o Radiopaedia: carregue a pasta DICOM ou as imagens e copie as notas de case.txt.

## Search on a DIMSE server (C-FIND).
# machine-drafted, needs native review
pacs-find-not-supported = { $host } não responde a pesquisas (C-FIND).
pacs-find-status = { $host } recusou a pesquisa (estado { $code }).

## Opening a series from a DIMSE server (C-GET, C-MOVE).
# machine-drafted, needs native review
pacs-retrieve-no-get = { $host } não envia séries com C-GET.
    Escolha C-MOVE nas definições do servidor.
# machine-drafted, needs native review
pacs-retrieve-no-move = { $host } não aceita pedidos C-MOVE.
# machine-drafted, needs native review
pacs-retrieve-unknown-destination = { $host } não conhece o nosso AE title { $ae }.
    Adicione-o no servidor com o endereço deste computador e a porta { $port }.
# machine-drafted, needs native review
pacs-retrieve-resources = { $host } está sem recursos. Tente novamente mais tarde.
# machine-drafted, needs native review
pacs-retrieve-status = { $host } recusou enviar a série (estado { $code }).
# machine-drafted, needs native review
pacs-retrieve-no-matches = { $host } não tem imagens para esta série.
# machine-drafted, needs native review
pacs-retrieve-roles = { $host } recusou enviar imagens por esta ligação.
    Escolha C-MOVE nas definições do servidor.
# machine-drafted, needs native review
pacs-retrieve-move-nothing = { $host } não conseguiu enviar as imagens para este computador.
    Verifique o endereço, a porta e a firewall, e se o servidor conhece o nosso AE title { $ae }.
# machine-drafted, needs native review
pacs-retrieve-nothing = { $host } não conseguiu enviar { $count ->
    [one] a imagem
    [many] nenhuma das { $count } imagens
   *[other] nenhuma das { $count } imagens
} num formato que o Specular consiga ler.
# machine-drafted, needs native review
pacs-retrieve-listen = Não é possível receber em { $address }, porta { $port }: { $reason }

## DICOM over TLS.
# machine-drafted, needs native review
pacs-fatal-tls-certificate = O certificado de { $host } não é fidedigno.
    O respetivo emissor tem de estar entre os certificados fidedignos do sistema.
pacs-fatal-tls = Sem ligação segura a { $host }.
    Verifique se a porta { $port } usa DICOM sobre TLS.
