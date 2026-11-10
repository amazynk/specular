# machine-drafted, needs native review
# Integração: abrir com, comandos do utilizador, colar (fase 2). Português europeu.
palette-open-with = Abrir com…
palette-paste = Colar
open-with-placeholder = Abrir com…
open-with-loading = A procurar aplicações…
open-with-default = { $name } (predefinida)
open-with-choose-other = Escolher outra aplicação…
open-with-choose-title = Escolher uma aplicação
open-with-failed = Não foi possível abrir com { $name }: { $reason }
command-missing = { $program } não encontrado
command-failed = Não foi possível iniciar { $name }: { $reason }
command-exited = { $name } terminou com um erro: { $status }
paste-nothing = Nada para abrir na área de transferência
paste-failed = Não foi possível colar a imagem: { $reason }
drag-failed = Não foi possível arrastar o ficheiro: { $reason }
about-storage = Definições: { $settings } · Cache: { $cache }
about-storage-portable = Modo portátil: definições e cache em { $dir }

## Scripting and recipes

palette-copy-view-command = Copiar vista como comando de shell
palette-copy-view-python = Copiar vista como Python
toast-copied-view-command = Comando copiado
script-copy-no-image = Nenhuma imagem para descrever
recipe-confirm-title = Executar a receita «{ $name }» pela primeira vez? Executa: { $command }
recipe-confirm-run = Executar
recipe-no-file = A receita precisa de um ficheiro de imagem
recipe-running = A executar { $name }…
recipe-done = { $name }: { $shapes ->
    [one] 1 forma adicionada
   *[other] { $shapes } formas adicionadas
}
recipe-failed = { $name } falhou: { $reason }
recipe-timeout = parado após { $seconds } s
recipe-no-output = { $name } não escreveu { $file }
