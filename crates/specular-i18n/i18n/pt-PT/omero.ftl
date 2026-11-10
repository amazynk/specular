# Specular — OMERO servers and the Image Data Resource (IDR), browsed read-only. Português (Portugal). machine-drafted, needs native review

palette-omero-servers = Servidores OMERO…

omero-title = Servidores OMERO
omero-servers-intro = Os projetos, rastreios e imagens de um servidor só são listados quando o explora. Nada é enviado antes.
omero-servers = Servidores
omero-servers-empty = Ainda não há servidores. Adicione um com o respetivo endereço OMERO.web.
omero-session-only = As palavras-passe só são guardadas até o Specular fechar.
omero-add = Adicionar…
omero-edit = Editar…
omero-remove = Remover
omero-close = Fechar
omero-browse = Explorar
omero-back = Voltar
omero-save = Guardar
omero-stop = Parar
omero-open = Abrir
# $server a server's name.
omero-removed = { $server } removido
# Shown with servers reached without encryption (http://).
omero-not-encrypted = Não encriptado
# A server browsed without signing in.
omero-public = Público
# $user the user name a server is signed in with.
omero-signed-in-as = Como { $user }

# The page that adds or edits a server.
omero-add-title = Adicionar um servidor
omero-edit-title = Definições do servidor
omero-field-name = Nome
omero-hint-name = Opcional
omero-field-url = Endereço
omero-hint-url = https://omero.example.org
omero-field-user = Nome de utilizador
omero-hint-user = Vazio para dados públicos
omero-field-password = Palavra-passe
omero-hint-password-kept = Deixe vazio para manter a já indicada
omero-password-session = A palavra-passe só é guardada até o Specular fechar, nunca no disco.
omero-password-needed = Introduza a palavra-passe: só é guardada até o Specular fechar.
omero-http-warning = Este endereço não é encriptado: a palavra-passe seria enviada em claro.
omero-url-invalid = Introduza o endereço OMERO.web, a começar por https:// ou http://.
# $reason why the list could not be saved.
omero-save-failed = Não foi possível guardar a lista de servidores: { $reason }

# The browse page.
omero-browse-title = Explorar
omero-top = Projetos e rastreios
omero-datasets = Conjuntos de dados
omero-plates = Placas
omero-wells = Poços
omero-fields = Campos
omero-images = Imagens
omero-kind-project = Projeto
omero-kind-dataset = Conjunto de dados
omero-kind-image = Imagem
omero-kind-screen = Rastreio
omero-kind-plate = Placa
omero-kind-well = Poço
# $well a well's row and column, as B7.
omero-well = Poço { $well }
omero-well-fields = { $count ->
    [one] Um campo
   *[other] { $count } campos
}
omero-children = { $count ->
    [one] Um item
   *[other] { $count } itens
}
omero-load-more = Carregar mais
omero-loading = A carregar…
omero-empty = Nada aqui
omero-stopped = Parado
omero-found = { $count ->
    [one] Um item
   *[other] { $count } itens
}
# $reason why the image could not be opened.
omero-open-failed = Não foi possível abrir a imagem: { $reason }
# The channel keys would hide the last channel shown.
omero-channel-last = Pelo menos um canal continua visível
# $server a server's name.
omero-session-ended = A sessão em { $server } terminou. Introduza a palavra-passe para continuar.

# Errors.
# $server a server's name.
omero-error-offline = Não foi possível contactar { $server }.
omero-error-sign-in = Este servidor exige iniciar sessão.
omero-error-expired = A sessão terminou.
omero-error-login = O nome de utilizador ou a palavra-passe foi recusado.
omero-error-forbidden = A sua conta pode não ter acesso a isto.
omero-error-not-found = Não encontrado no servidor (pode ter sido removido).
# $code an HTTP status code.
omero-error-status = O servidor respondeu com o erro { $code }.
# $reason what was wrong with the answer.
omero-error-malformed = Não foi possível ler a resposta do servidor: { $reason }
