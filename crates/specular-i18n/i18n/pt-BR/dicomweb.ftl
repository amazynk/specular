# Specular — DICOMweb servers, study search and streamed series (DESIGN.md §12.8). Português (Brasil).

palette-dicom-servers = Servidores DICOMweb…

dicomweb-title = Servidores DICOMweb
dicomweb-servers-intro = Os estudos de um servidor só são pesquisados e abertos quando você pede. Nada é enviado antes.
dicomweb-servers = Servidores
dicomweb-servers-empty = Nenhum servidor ainda. Adicione um com o endereço DICOMweb dele.
dicomweb-session-only = Senhas e tokens são mantidos somente até o Specular ser fechado.
dicomweb-add = Adicionar…
dicomweb-edit = Editar…
dicomweb-remove = Remover
dicomweb-close = Fechar
dicomweb-browse = Navegar
dicomweb-back = Voltar
dicomweb-save = Salvar
dicomweb-search = Pesquisar
dicomweb-stop = Parar
dicomweb-removed = { $server } removido
# machine-drafted, needs native review
dicomweb-remove-ask = Remover “{ $server }” dos servidores salvos?
dicomweb-remove-detail = Nada no disco é perdido. A senha ou o token dele nesta sessão são esquecidos.
# Servidores acessados sem criptografia (http://).
dicomweb-not-encrypted = Sem criptografia

# Página para adicionar ou editar um servidor.
dicomweb-add-title = Adicionar um servidor
dicomweb-edit-title = Configurações do servidor
dicomweb-field-name = Nome
dicomweb-hint-name = Opcional
dicomweb-field-url = Endereço
dicomweb-hint-url = https://pacs.example.org/dicom-web
dicomweb-field-auth = Login
dicomweb-auth-none = Sem login
dicomweb-auth-basic = Nome de usuário e senha
dicomweb-auth-bearer = Token de acesso
dicomweb-field-user = Nome de usuário
dicomweb-field-password = Senha
dicomweb-field-token = Token de acesso
dicomweb-hint-secret-kept = Deixe em branco para manter o atual
dicomweb-secret-session = A senha ou o token é mantido somente até o Specular ser fechado, nunca no disco.
dicomweb-secret-needed = Digite a senha ou o token novamente: ele é mantido somente até o Specular ser fechado.
dicomweb-http-warning = Este endereço não é criptografado: a senha ou o token seria enviado sem proteção.
dicomweb-url-empty = Digite o endereço DICOMweb do servidor.
dicomweb-url-scheme = O endereço deve começar com https:// ou http://.
dicomweb-url-host = O endereço não tem nome de servidor.
dicomweb-url-userinfo = Coloque o nome de usuário e a senha nos campos de login, não no endereço.
dicomweb-save-failed = Não foi possível salvar a lista de servidores: { $reason }

# Página de pesquisa.
dicomweb-search-title = Encontrar estudos
dicomweb-field-patient-name = Nome do paciente
dicomweb-field-patient-id = ID do paciente
dicomweb-field-date-from = De
dicomweb-field-date-to = Até
dicomweb-hint-date = AAAAMMDD
dicomweb-field-modality = Modalidade
dicomweb-hint-modality = CT, MR…
dicomweb-field-accession = Número de acesso
dicomweb-show-patient = Mostrar dados do paciente
dicomweb-switch-on = Ativado
dicomweb-switch-off = Desativado
dicomweb-patient-shown = Dados do paciente visíveis
dicomweb-patient-hidden = Dados do paciente ocultos
dicomweb-studies = Estudos
dicomweb-search-hint = Preencha qualquer um dos campos e pesquise.
dicomweb-no-studies = Nenhum estudo corresponde.
dicomweb-searching = Pesquisando…
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
dicomweb-error-offline = Não é possível acessar { $server }. Verifique a rede e o endereço.
dicomweb-error-auth = O servidor recusou o nome de usuário, a senha ou o token.
dicomweb-error-not-found = Não há serviço DICOMweb neste endereço.
dicomweb-error-other = O servidor não conseguiu responder: { $reason }

# Série transmitida para o visualizador.
dicomweb-loading = Carregando { $series }…
dicomweb-loading-progress = Carregando { $series }: { $done } de { $total }
dicomweb-loaded = { $series }: { $total ->
    [one] uma imagem carregada
   *[other] todas as { $total } imagens carregadas
}
dicomweb-loaded-failed = Não foi possível carregar { $failed } de { $total } imagens.
dicomweb-offline-cached = Não é possível acessar { $server }: mostrando as imagens salvas anteriormente.
dicomweb-cache-failed = Não foi possível abrir a pasta de downloads: { $reason }
