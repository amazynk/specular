# Specular — OMERO servers and the Image Data Resource (IDR), browsed read-only. Português (Brasil). machine-drafted, needs native review

palette-omero-servers = Servidores OMERO…

omero-title = Servidores OMERO
omero-servers-intro = Projetos, triagens e imagens de um servidor só são listados quando você navega por ele. Nada é enviado antes.
omero-servers = Servidores
omero-servers-empty = Nenhum servidor ainda. Adicione um com o endereço OMERO.web.
omero-session-only = As senhas só são mantidas até o Specular fechar.
omero-add = Adicionar…
omero-edit = Editar…
omero-remove = Remover
omero-close = Fechar
omero-browse = Navegar
omero-back = Voltar
omero-save = Salvar
omero-stop = Parar
omero-open = Abrir
# $server a server's name.
omero-removed = { $server } removido
# Shown with servers reached without encryption (http://).
omero-not-encrypted = Não criptografado
# A server browsed without signing in.
omero-public = Público
# $user the user name a server is signed in with.
omero-signed-in-as = Como { $user }

# The page that adds or edits a server.
omero-add-title = Adicionar um servidor
omero-edit-title = Configurações do servidor
omero-field-name = Nome
omero-hint-name = Opcional
omero-field-url = Endereço
omero-hint-url = https://omero.example.org
omero-field-user = Nome de usuário
omero-hint-user = Vazio para dados públicos
omero-field-password = Senha
omero-hint-password-kept = Deixe vazio para manter a já informada
omero-password-session = A senha só é mantida até o Specular fechar, nunca no disco.
omero-password-needed = Digite a senha: ela só é mantida até o Specular fechar.
omero-http-warning = Este endereço não é criptografado: a senha seria enviada em texto puro.
omero-url-invalid = Digite o endereço OMERO.web, começando com https:// ou http://.
# $reason why the list could not be saved.
omero-save-failed = Não foi possível salvar a lista de servidores: { $reason }

# The browse page.
omero-browse-title = Navegar
omero-top = Projetos e triagens
omero-datasets = Conjuntos de dados
omero-plates = Placas
omero-wells = Poços
omero-fields = Campos
omero-images = Imagens
omero-kind-project = Projeto
omero-kind-dataset = Conjunto de dados
omero-kind-image = Imagem
omero-kind-screen = Triagem
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
omero-loading = Carregando…
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
omero-session-ended = A sessão em { $server } terminou. Digite a senha para continuar.

# Errors.
# $server a server's name.
omero-error-offline = Não foi possível acessar { $server }.
omero-error-sign-in = Este servidor exige login.
omero-error-expired = A sessão terminou.
omero-error-login = O nome de usuário ou a senha foi recusado.
omero-error-forbidden = Sua conta pode não ter acesso a isto.
omero-error-not-found = Não encontrado no servidor (pode ter sido removido).
# $code an HTTP status code.
omero-error-status = O servidor respondeu com o erro { $code }.
# $reason what was wrong with the answer.
omero-error-malformed = Não foi possível ler a resposta do servidor: { $reason }
