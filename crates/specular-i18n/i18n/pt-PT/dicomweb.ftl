# Specular — DICOMweb servers, study search and streamed series (DESIGN.md §12.8). Português (Portugal).

palette-dicom-servers = Servidores DICOMweb…

dicomweb-title = Servidores DICOMweb
dicomweb-servers-intro = Os estudos de um servidor só são pesquisados e abertos quando o pedir. Nada é enviado antes.
dicomweb-servers = Servidores
dicomweb-servers-empty = Ainda não há servidores. Adicione um com o respetivo endereço DICOMweb.
dicomweb-session-only = As palavras-passe e os tokens só são guardados até o Specular ser fechado.
dicomweb-add = Adicionar…
dicomweb-edit = Editar…
dicomweb-remove = Remover
dicomweb-close = Fechar
dicomweb-browse = Explorar
dicomweb-back = Voltar
dicomweb-save = Guardar
dicomweb-search = Pesquisar
dicomweb-stop = Parar
dicomweb-removed = { $server } removido
# machine-drafted, needs native review
dicomweb-remove-ask = Remover «{ $server }» dos servidores guardados?
dicomweb-remove-detail = Nada se perde no disco. A palavra-passe ou o token dele nesta sessão são esquecidos.
# Servidores acedidos sem encriptação (http://).
dicomweb-not-encrypted = Sem encriptação

# Página para adicionar ou editar um servidor.
dicomweb-add-title = Adicionar um servidor
dicomweb-edit-title = Definições do servidor
dicomweb-field-name = Nome
dicomweb-hint-name = Opcional
dicomweb-field-url = Endereço
dicomweb-hint-url = https://pacs.example.org/dicom-web
dicomweb-field-auth = Início de sessão
dicomweb-auth-none = Sem início de sessão
dicomweb-auth-basic = Nome de utilizador e palavra-passe
dicomweb-auth-bearer = Token de acesso
dicomweb-field-user = Nome de utilizador
dicomweb-field-password = Palavra-passe
dicomweb-field-token = Token de acesso
dicomweb-hint-secret-kept = Deixe vazio para manter o atual
dicomweb-secret-session = A palavra-passe ou o token só é guardado até o Specular ser fechado, nunca no disco.
dicomweb-secret-needed = Introduza novamente a palavra-passe ou o token: só é guardado até o Specular ser fechado.
dicomweb-http-warning = Este endereço não está encriptado: a palavra-passe ou o token seria enviado sem proteção.
dicomweb-url-empty = Introduza o endereço DICOMweb do servidor.
dicomweb-url-scheme = O endereço tem de começar por https:// ou http://.
dicomweb-url-host = O endereço não tem nome de servidor.
dicomweb-url-userinfo = Coloque o nome de utilizador e a palavra-passe nos campos de início de sessão, não no endereço.
dicomweb-save-failed = Não foi possível guardar a lista de servidores: { $reason }

# Página de pesquisa.
dicomweb-search-title = Encontrar estudos
dicomweb-field-patient-name = Nome do doente
dicomweb-field-patient-id = ID do doente
dicomweb-field-date-from = De
dicomweb-field-date-to = Até
dicomweb-hint-date = AAAAMMDD
dicomweb-field-modality = Modalidade
dicomweb-hint-modality = CT, MR…
dicomweb-field-accession = Número de acesso
dicomweb-show-patient = Mostrar dados do doente
dicomweb-switch-on = Ativado
dicomweb-switch-off = Desativado
dicomweb-patient-shown = Dados do doente visíveis
dicomweb-patient-hidden = Dados do doente ocultos
dicomweb-studies = Estudos
dicomweb-search-hint = Preencha qualquer um dos campos e pesquise.
dicomweb-no-studies = Nenhum estudo corresponde.
dicomweb-searching = A pesquisar…
dicomweb-stopped = Parado
dicomweb-load-more = Carregar mais
dicomweb-study-untitled = Estudo sem descrição
dicomweb-series-untitled = Série sem descrição
dicomweb-study-counts = { $series ->
    [one] Uma série
   *[other] { $series } séries
}, { $images ->
    [one] uma imagem
   *[other] { $images } imagens
}
dicomweb-series-images = { $count ->
    [one] Uma imagem
   *[other] { $count } imagens
}
dicomweb-patient-id = ID { $id }
dicomweb-accession = Acesso { $number }
dicomweb-found = { $count ->
    [one] Um estudo encontrado
   *[other] { $count } estudos encontrados
}
dicomweb-series-found = { $count ->
    [one] Uma série
   *[other] { $count } séries
}

# Erros de comunicação com um servidor.
dicomweb-error-offline = Não é possível aceder a { $server }. Verifique a rede e o endereço.
dicomweb-error-auth = O servidor recusou o nome de utilizador, a palavra-passe ou o token.
dicomweb-error-not-found = Não existe nenhum serviço DICOMweb neste endereço.
dicomweb-error-other = O servidor não conseguiu responder: { $reason }

# Série transmitida para o visualizador.
dicomweb-loading = A carregar { $series }…
dicomweb-loading-progress = A carregar { $series }: { $done } de { $total }
dicomweb-loaded = { $series }: { $total ->
    [one] uma imagem carregada
   *[other] todas as { $total } imagens carregadas
}
dicomweb-loaded-failed = Não foi possível carregar { $failed } de { $total } imagens.
dicomweb-offline-cached = Não é possível aceder a { $server }: a mostrar as imagens guardadas anteriormente.
dicomweb-cache-failed = Não foi possível abrir a pasta de transferências: { $reason }
