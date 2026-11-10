# machine-drafted, needs native review
## O painel do manual, relatórios de falha, o pacote de diagnóstico, atualizações e Acerca de. Português (Portugal).
## Estilo: curto, claro, sem enchimento; uma ideia por frase.

## Paleta

palette-help-manual = Ajuda
palette-help-manual-help = Abrir o manual na página do que está no ecrã.
palette-save-diagnostics = Guardar pacote de diagnóstico…
palette-save-diagnostics-help = Guardar relatórios de falha, o registo e detalhes do sistema num ficheiro zip. Nada é enviado.
# Entrada da paleta que abre uma secção do manual; { $title } é uma página ou «Página › Secção».
palette-help-topic = Ajuda: { $title }
palette-help-topic-help = Abrir esta secção do manual.
# Sob um comando indisponível na paleta.
palette-disabled-help = Indisponível agora. Prima Enter para saber porquê.

## O painel de ajuda

help-title = Ajuda
help-search-placeholder = Pesquisar no manual
help-contents = Índice
help-back = Retroceder
help-forward = Avançar
help-no-results = Nada encontrado para «{ $query }».
help-results = { $count ->
    [one] { $count } secção encontrada
    [many] { $count } secções encontradas
   *[other] { $count } secções encontradas
}
help-section-start = Primeiros passos
help-section-modes = Visualização e modos
help-section-tasks = Como…
help-section-reference = Referência
help-keys-unbound = Não definido
# Um comando citado no manual sem tecla; é executado a partir da paleta.
help-key-unbound = { $name } (paleta)
help-keys-command = Comando
help-keys-keys = Teclas
help-commands = Todos os comandos estão na paleta de comandos ({ $key }). Escreva parte do nome.
help-commands-no-key = Todos os comandos estão na paleta de comandos. Escreva parte do nome.
help-link = Ir para { $title }
help-open = Ajuda
# Nome do painel para leitores de ecrã; { $page } é a página apresentada.
help-label = Ajuda: { $page }
help-disabled-why = Porquê?

## Relatório de falha oferecido no próximo arranque

crash-title = O { -app-name } fechou inesperadamente
crash-body = Foi guardado um relatório neste computador. Nada foi enviado.
crash-view = Ver relatório
crash-hide = Ocultar relatório
crash-copy = Copiar
crash-copied = Relatório copiado
crash-save-bundle = Guardar pacote de diagnóstico…
crash-dismiss = Ignorar

## Pacote de diagnóstico

diag-title = Guardar pacote de diagnóstico
diag-intro = Um ficheiro zip para anexar a um relatório de erro. É guardado neste computador. Nada é enviado.
diag-contents = Conteúdo
diag-include-names = Incluir nomes de ficheiros
diag-include-names-help = Desativado: os nomes são substituídos por file-1.jpg, dir-1, etc.
diag-privacy = A sua pasta pessoal aparece como ~ e o seu nome de utilizador como <user>. Palavras-passe, endereços de servidor e imagens nunca são incluídos.
diag-where = Comunicar um problema: { $issues } · { $email }
diag-save = Guardar…
diag-cancel = Cancelar
diag-saved = Pacote de diagnóstico guardado: { $path }
diag-failed = Não foi possível guardar o pacote de diagnóstico: { $reason }
diag-entry-summary = Versão, sistema, GPU e controlador
diag-entry-crash = Último relatório de falha
diag-entry-crashes = Relatórios de falha anteriores ({ $count })
diag-entry-log = Últimas { $count } linhas do registo
diag-entry-settings = Definições, sem palavras-passe nem endereços
diag-entry-gpu = Detalhes da GPU e do ecrã
diag-entry-readme = O que é este pacote
diag-size = { $size }
# O README.txt dentro do pacote.
diag-readme =
    Pacote de diagnóstico do { -app-name }.

    Contém o que ajuda a encontrar um problema: versão e detalhes do sistema, relatórios de falha, o fim do registo e as definições.

    A sua pasta pessoal aparece como ~ e o seu nome de utilizador como <user>. Salvo se escolheu incluí-los, os nomes de ficheiros e pastas são substituídos por file-1, dir-1, etc. As palavras-passe e os endereços de servidor são removidos.

    Nada foi enviado. É o utilizador que decide se partilha este ficheiro.

## Atualizações (Definições)

settings-section-updates = Atualizações
settings-updates-check = Procurar atualizações
settings-updates-check-help = Uma vez por dia, perguntar se existe uma versão nova. O pedido leva apenas o número da versão: nenhum identificador, nenhum cookie. Desativado: o { -app-name } nunca se liga para atualizações.
settings-updates-channel = Canal
settings-updates-channel-help = Estável: apenas versões publicadas. Beta: também versões de teste.
settings-updates-channel-stable = Estável
settings-updates-channel-beta = Beta
settings-updates-check-now = Verificar agora
settings-updates-check-now-help = Verificar uma vez agora.
update-available = A versão { $version } está disponível
update-notes = Notas de versão
update-download = Transferir
update-none = O { -app-name } está atualizado.
update-checking = A procurar atualizações…
update-failed = Não foi possível procurar atualizações: { $reason }
update-last-checked = Última verificação: { $when }

## Acerca de

about-build = Build { $commit } · { $profile } · { $target }
about-build-no-commit = Build { $profile } · { $target }
about-help = Ajuda
about-diagnostics = Guardar pacote de diagnóstico…
# machine-drafted, needs native review
about-renderer = Informações do renderizador…
about-licences-count = { $count ->
    [one] { $count } componente de terceiros
    [many] { $count } componentes de terceiros
   *[other] { $count } componentes de terceiros
}
about-licence-group = { $licence } ({ $count })
about-contact = Problemas: { $issues } · Suporte: { $email }
