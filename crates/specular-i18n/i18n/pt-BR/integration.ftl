# machine-drafted, needs native review
# Integração: abrir com, comandos do usuário, colar (Fase 2). Português do Brasil.
palette-open-with = Abrir com…
palette-paste = Colar
open-with-placeholder = Abrir com…
open-with-loading = Procurando aplicativos…
open-with-default = { $name } (padrão)
open-with-choose-other = Escolher outro aplicativo…
open-with-choose-title = Escolha um aplicativo
open-with-failed = Não foi possível abrir com { $name }: { $reason }
command-missing = { $program } não encontrado
command-failed = Não foi possível iniciar { $name }: { $reason }
command-exited = { $name } terminou com um erro: { $status }
paste-nothing = Nada para abrir na área de transferência
paste-failed = Não foi possível colar a imagem: { $reason }
drag-failed = Não foi possível arrastar o arquivo: { $reason }
about-storage = Configurações: { $settings } · Cache: { $cache }
about-storage-portable = Modo portátil: configurações e cache em { $dir }

## Scripting and recipes

palette-copy-view-command = Copiar visualização como comando de shell
palette-copy-view-python = Copiar visualização como Python
toast-copied-view-command = Comando copiado
script-copy-no-image = Nenhuma imagem para descrever
recipe-confirm-title = Executar a receita “{ $name }” pela primeira vez? Ela executa: { $command }
recipe-confirm-run = Executar
recipe-no-file = A receita precisa de um arquivo de imagem
recipe-running = Executando { $name }…
recipe-done = { $name }: { $shapes ->
    [one] { $shapes } forma adicionada
   *[other] { $shapes } formas adicionadas
}
recipe-failed = { $name } falhou: { $reason }
recipe-timeout = interrompido após { $seconds } s
recipe-no-output = { $name } não gravou { $file }
