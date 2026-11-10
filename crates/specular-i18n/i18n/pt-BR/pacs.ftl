# machine-drafted, needs native review

## Send to PACS (C-STORE, STOW-RS), DIMSE servers, teaching quiz and teaching case export.

## Servers: DIMSE fields and Verify.

dicomweb-field-kind = Tipo
dicomweb-kind-web = DICOMweb
dicomweb-kind-dimse = DICOM (DIMSE)
dicomweb-field-host = Host
dicomweb-hint-host = pacs.example.org
dicomweb-field-port = Porta
dicomweb-hint-port = 104 ou 11112
# The application entity title the server answers to.
dicomweb-field-ae = AE title do servidor
dicomweb-hint-ae = PACS
# The application entity title Specular sends as.
dicomweb-field-our-ae = Nosso AE title
# machine-drafted, needs native review
dicomweb-field-tls = Conexão segura (TLS)
dicomweb-tls-on = Conexão segura ativada
dicomweb-tls-off = Conexão segura desativada
dicomweb-problem-host = Digite o nome do host ou o endereço do servidor.
dicomweb-problem-port = A porta é um número de 1 a 65535.
dicomweb-problem-ae = Um AE title tem de 1 a 16 caracteres, sem barra invertida.
# machine-drafted, needs native review
dicomweb-dimse-note-open = Servidores DICOM (DIMSE) podem ser pesquisados, abertos e receber envios.
# How series are fetched: C-GET (over our own connection) or C-MOVE (the server connects back).
# machine-drafted, needs native review
pacs-field-retrieve = Recuperar com
# machine-drafted, needs native review
pacs-retrieve-get = C-GET
# machine-drafted, needs native review
pacs-retrieve-move = C-MOVE
# C-MOVE: the address of this computer and the port the server sends to.
# machine-drafted, needs native review
pacs-field-receive-address = Receber no endereço
# machine-drafted, needs native review
pacs-hint-receive-address = Endereço deste computador, como 192.168.1.20
# machine-drafted, needs native review
pacs-field-receive-port = Receber na porta
# machine-drafted, needs native review
pacs-hint-receive-port = Como 11113
# machine-drafted, needs native review
pacs-move-note = Para C-MOVE, o servidor precisa conhecer nosso AE title { $ae } com este endereço e esta porta.
    O Specular só escuta ali enquanto uma série está chegando.
# machine-drafted, needs native review
pacs-problem-receive-address = Digite o endereço deste computador (como 192.168.1.20) para receber.
# machine-drafted, needs native review
pacs-problem-receive-port = Digite uma porta para receber. O C-MOVE precisa de uma.
dicomweb-verify = Verificar
dicomweb-verifying = Verificando o servidor…
dicomweb-verify-ok = { $server } respondeu.

## The Studies browser and the palette.

studies-send = Enviar ao PACS…
studies-send-description = Envia esta série, o estudo dela ou tudo o que está no disco a um servidor.
palette-send-to-pacs = Enviar ao PACS…
palette-send-to-pacs-help = Enviar a série ou o estudo atual a um servidor PACS.

## The Send page.

pacs-title = Enviar ao PACS
# Under the title: what is being sent.
pacs-summary = { $studies ->
    [one] { $studies } estudo
    [many] { $studies } estudos
   *[other] { $studies } estudos
} · { $files ->
    [one] { $files } arquivo
    [many] { $files } arquivos
   *[other] { $files } arquivos
}
pacs-what = Enviar
pacs-scope-series = Esta série ({ $files ->
    [one] { $files } arquivo
    [many] { $files } arquivos
   *[other] { $files } arquivos
})
pacs-scope-study = Este estudo ({ $files ->
    [one] { $files } arquivo
    [many] { $files } arquivos
   *[other] { $files } arquivos
})
pacs-scope-all = Tudo o que foi lido ({ $studies ->
    [one] { $studies } estudo
    [many] { $studies } estudos
   *[other] { $studies } estudos
}, { $files ->
    [one] { $files } arquivo
    [many] { $files } arquivos
   *[other] { $files } arquivos
})
pacs-anonymise = Anonimizar
pacs-anonymise-note = Os dados do paciente são removidos das cópias enviadas. Seus arquivos não são alterados.
pacs-identified-note = Os arquivos são enviados como estão, com os dados do paciente.
# The list of servers to choose from.
pacs-to = Para
pacs-no-servers = Nenhum servidor ainda. Adicione um em Servidores DICOMweb.
pacs-send = Enviar…
pacs-go-back = Voltar
pacs-confirm-send = Enviar
pacs-confirm = Enviar { $studies ->
    [one] { $studies } estudo
    [many] { $studies } estudos
   *[other] { $studies } estudos
} ({ $files ->
    [one] { $files } arquivo
    [many] { $files } arquivos
   *[other] { $files } arquivos
}) para { $server }?
pacs-busy = Ainda há um envio em andamento.
pacs-started = Enviando.
pacs-stopping = Parando…
pacs-sending = Enviando { $done } de { $total } para { $server }…
pacs-sent-all = { $total ->
    [one] { $total } arquivo foi enviado para { $server }.
    [many] Todos os { $total } arquivos foram enviados para { $server }.
   *[other] Todos os { $total } arquivos foram enviados para { $server }.
}
pacs-sent-some = { $sent } de { $total } arquivos foram enviados para { $server }.
pacs-stopped = Parado após { $sent } de { $total ->
        [one] { $total } arquivo
       *[other] { $total } arquivos
    }.
pacs-stopped-because = Parado após { $sent } de { $total ->
        [one] { $total } arquivo
       *[other] { $total } arquivos
    }: { $reason }
pacs-warnings = { $count ->
    [one] { $count } arquivo foi armazenado com um aviso.
    [many] { $count } arquivos foram armazenados com um aviso.
   *[other] { $count } arquivos foram armazenados com um aviso.
}
# A file that was not sent: its file name and why.
pacs-failed-line = { $name }: { $reason }
pacs-failed-more = E mais { $count }.

## Why one file was not sent.

pacs-reason-unreadable = Não foi possível ler: { $reason }
pacs-reason-not-dicom = Não é um arquivo DICOM
pacs-reason-burned-in = Tem dados do paciente gravados na imagem, então não é enviado anonimizado
pacs-reason-document = Um documento que não pode ser anonimizado
pacs-reason-anonymise = Não foi possível anonimizar: { $reason }
pacs-reason-not-accepted = O servidor não aceita { $class }
pacs-reason-convert = Não foi possível converter para o servidor: { $reason }
pacs-reason-resources = O servidor está sem espaço ou recursos
pacs-reason-mismatch = O servidor diz que os dados não correspondem ao tipo
pacs-reason-understand = O servidor não conseguiu lê-lo
pacs-reason-class = O servidor não suporta este tipo de objeto
pacs-reason-not-authorised = O servidor não permite
pacs-reason-status = O servidor recusou (status { $code })
pacs-reason-refused = O servidor recusou
pacs-reason-too-large = Grande demais para o servidor
pacs-reason-http = O servidor recusou (erro { $status })

## Why sending stopped, or Verify failed.

pacs-fatal-network-off = A rede está desligada.
pacs-fatal-unknown-host = { $host } não é um nome de host conhecido.
pacs-fatal-connect = Nada responde em { $host }, porta { $port }.
pacs-fatal-unreachable = Não é possível acessar { $host }.
pacs-fatal-timeout = { $host } não respondeu a tempo.
pacs-fatal-cancelled = Parado.
pacs-fatal-called-ae = { $host } não responde ao AE title { $ae }. Verifique o AE title do servidor.
pacs-fatal-calling-ae = { $host } não conhece nosso AE title { $ae }. Adicione-o no servidor.
pacs-fatal-busy = { $host } está ocupado. Tente novamente mais tarde.
pacs-fatal-rejected = { $host } recusou a conexão.
pacs-fatal-aborted = { $host } encerrou a conexão.
pacs-fatal-nothing-accepted = { $host } não aceita nenhum destes tipos de objeto.
pacs-fatal-protocol = { $host } respondeu de um jeito que o Specular não entende.
pacs-fatal-auth = { $host } não aceitou o login.
pacs-fatal-http = { $host } respondeu com o erro { $status }.

## Teaching quiz.

palette-teaching-quiz = Quiz de ensino
palette-teaching-quiz-help = Percorrer as imagens-chave ou um caso de ensino, com os achados ocultos até você revelá-los.
# "Case 2 of 5".
quiz-case = Caso { $n } de { $total }
quiz-hint-hidden = Espaço mostra os achados · → próximo caso · Esc encerra
quiz-hint-revealed = Espaço os oculta de novo · → próximo caso · Esc encerra
quiz-hidden = Achados ocultos.
quiz-no-findings = Nenhum achado registrado.
quiz-notes = Notas do caso: { $notes }
quiz-ended = Quiz encerrado.
quiz-empty = Não há casos para mostrar. Marque imagens-chave com K ou escolha uma pasta de caso de ensino.
quiz-not-a-case = Esta pasta não é um caso de ensino exportado pelo Specular.
quiz-pick-title = Escolher uma pasta de caso de ensino

## Teaching case folder (Export key images).

figure-keys-case = Pasta de caso de ensino
figure-case-notes = Notas do caso
figure-case-findings = Achados
figure-case-none = Nenhum registrado
figure-case-readme = Caso de ensino exportado pelo Specular. Os dados do paciente foram removidos.
    images: as imagens-chave, com o texto gravado tarjado em preto.
    DICOM: cópias anonimizadas e um manifesto Key Object Selection "For Teaching".
    case.txt: as notas do caso e os achados de cada imagem. case.json: o mesmo para scripts.
    Para o Radiopaedia: envie a pasta DICOM ou as imagens e copie as notas de case.txt.

## Search on a DIMSE server (C-FIND).
# machine-drafted, needs native review
pacs-find-not-supported = { $host } não responde a pesquisas (C-FIND).
pacs-find-status = { $host } recusou a pesquisa (status { $code }).

## Opening a series from a DIMSE server (C-GET, C-MOVE).
# machine-drafted, needs native review
pacs-retrieve-no-get = { $host } não envia séries com C-GET.
    Escolha C-MOVE nas configurações do servidor.
# machine-drafted, needs native review
pacs-retrieve-no-move = { $host } não aceita solicitações C-MOVE.
# machine-drafted, needs native review
pacs-retrieve-unknown-destination = { $host } não conhece nosso AE title { $ae }.
    Adicione-o no servidor com o endereço deste computador e a porta { $port }.
# machine-drafted, needs native review
pacs-retrieve-resources = { $host } está sem recursos. Tente novamente mais tarde.
# machine-drafted, needs native review
pacs-retrieve-status = { $host } recusou enviar a série (status { $code }).
# machine-drafted, needs native review
pacs-retrieve-no-matches = { $host } não tem imagens para esta série.
# machine-drafted, needs native review
pacs-retrieve-roles = { $host } recusou enviar imagens por esta conexão.
    Escolha C-MOVE nas configurações do servidor.
# machine-drafted, needs native review
pacs-retrieve-move-nothing = { $host } não conseguiu enviar as imagens para este computador.
    Verifique o endereço, a porta e o firewall, e se o servidor conhece nosso AE title { $ae }.
# machine-drafted, needs native review
pacs-retrieve-nothing = { $host } não conseguiu enviar { $count ->
    [one] a imagem
    [many] nenhuma das { $count } imagens
   *[other] nenhuma das { $count } imagens
} em um formato que o Specular lê.
# machine-drafted, needs native review
pacs-retrieve-listen = Não é possível receber em { $address }, porta { $port }: { $reason }

## DICOM over TLS.
# machine-drafted, needs native review
pacs-fatal-tls-certificate = O certificado de { $host } não é confiável.
    O emissor dele precisa estar entre os certificados confiáveis do sistema.
pacs-fatal-tls = Não há conexão segura com { $host }.
    Verifique se a porta { $port } usa DICOM sobre TLS.
