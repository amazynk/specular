# machine-drafted, needs native review
## O painel do manual, relatórios de falha, o pacote de diagnóstico, atualizações e Sobre. Português (Brasil).
## Estilo: curto, claro, sem enchimento; uma ideia por frase.

## Paleta

palette-help-manual = Ajuda
palette-help-manual-help = Abrir o manual na página do que está na tela.
palette-save-diagnostics = Salvar pacote de diagnóstico…
palette-save-diagnostics-help = Salvar relatórios de falha, o log e detalhes do sistema em um arquivo zip. Nada é enviado.
# Item da paleta que abre uma seção do manual; { $title } é uma página ou “Página › Seção”.
palette-help-topic = Ajuda: { $title }
palette-help-topic-help = Abrir esta seção do manual.
# Sob um comando indisponível na paleta.
palette-disabled-help = Indisponível agora. Pressione Enter para saber por quê.

## O painel de ajuda

help-title = Ajuda
help-search-placeholder = Pesquisar no manual
help-contents = Sumário
help-back = Voltar
help-forward = Avançar
help-no-results = Nada encontrado para “{ $query }”.
help-results = { $count ->
    [one] { $count } seção encontrada
    [many] { $count } seções encontradas
   *[other] { $count } seções encontradas
}
help-section-start = Primeiros passos
help-section-modes = Visualização e modos
help-section-tasks = Como faço…
help-section-reference = Referência
help-keys-unbound = Não definido
# Um comando citado no manual sem tecla; ele é executado pela paleta.
help-key-unbound = { $name } (paleta)
help-keys-command = Comando
help-keys-keys = Teclas
help-commands = Todos os comandos estão na paleta de comandos ({ $key }). Digite parte do nome.
help-commands-no-key = Todos os comandos estão na paleta de comandos. Digite parte do nome.
help-link = Ir para { $title }
help-open = Ajuda
# Nome do painel para leitores de tela; { $page } é a página exibida.
help-label = Ajuda: { $page }
help-disabled-why = Por quê?

## Relatório de falha oferecido na próxima inicialização

crash-title = O { -app-name } fechou inesperadamente
crash-body = Um relatório foi salvo neste computador. Nada foi enviado.
crash-view = Ver relatório
crash-hide = Ocultar relatório
crash-copy = Copiar
crash-copied = Relatório copiado
crash-save-bundle = Salvar pacote de diagnóstico…
crash-dismiss = Dispensar

## Pacote de diagnóstico

diag-title = Salvar pacote de diagnóstico
diag-intro = Um arquivo zip para anexar a um relatório de bug. Ele é salvo neste computador. Nada é enviado.
diag-contents = Conteúdo
diag-include-names = Incluir nomes de arquivos
diag-include-names-help = Desativado: os nomes são substituídos por file-1.jpg, dir-1 e assim por diante.
diag-privacy = Sua pasta pessoal aparece como ~ e seu nome de usuário como <user>. Senhas, endereços de servidor e imagens nunca são incluídos.
diag-where = Relatar um problema: { $issues } · { $email }
diag-save = Salvar…
diag-cancel = Cancelar
diag-saved = Pacote de diagnóstico salvo: { $path }
diag-failed = Não foi possível salvar o pacote de diagnóstico: { $reason }
diag-entry-summary = Versão, sistema, GPU e driver
diag-entry-crash = Último relatório de falha
diag-entry-crashes = Relatórios de falha anteriores ({ $count })
diag-entry-log = Últimas { $count } linhas do log
diag-entry-settings = Configurações, sem senhas nem endereços
diag-entry-gpu = Detalhes da GPU e da tela
diag-entry-readme = O que é este pacote
diag-size = { $size }
# O README.txt dentro do pacote.
diag-readme =
    Pacote de diagnóstico do { -app-name }.

    Contém o que ajuda a encontrar um problema: versão e detalhes do sistema, relatórios de falha, o final do log e as configurações.

    Sua pasta pessoal aparece como ~ e seu nome de usuário como <user>. A menos que você tenha escolhido incluí-los, os nomes de arquivos e pastas são substituídos por file-1, dir-1 e assim por diante. Senhas e endereços de servidor são removidos.

    Nada foi enviado. Você decide se compartilha este arquivo.

## Atualizações (Configurações)

settings-section-updates = Atualizações
settings-updates-check = Procurar atualizações
settings-updates-check-help = Uma vez por dia, perguntar se existe uma versão nova. A solicitação leva apenas o número da versão: nenhum identificador, nenhum cookie. Desativado: o { -app-name } nunca se conecta para atualizações.
settings-updates-channel = Canal
settings-updates-channel-help = Estável: apenas versões lançadas. Beta: também versões de teste.
settings-updates-channel-stable = Estável
settings-updates-channel-beta = Beta
settings-updates-check-now = Verificar agora
settings-updates-check-now-help = Verificar uma vez agora.
update-available = A versão { $version } está disponível
update-notes = Notas da versão
update-download = Baixar
update-none = O { -app-name } está atualizado.
update-checking = Procurando atualizações…
update-failed = Não foi possível procurar atualizações: { $reason }
update-last-checked = Última verificação: { $when }

## Sobre

about-build = Build { $commit } · { $profile } · { $target }
about-build-no-commit = Build { $profile } · { $target }
about-help = Ajuda
about-diagnostics = Salvar pacote de diagnóstico…
# machine-drafted, needs native review
about-renderer = Informações do renderizador…
about-licences-count = { $count ->
    [one] { $count } componente de terceiros
    [many] { $count } componentes de terceiros
   *[other] { $count } componentes de terceiros
}
about-licence-group = { $licence } ({ $count })
about-contact = Problemas: { $issues } · Suporte: { $email }
