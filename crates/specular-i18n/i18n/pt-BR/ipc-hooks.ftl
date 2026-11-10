# machine-drafted, needs native review
# Remote control and events (Settings → Integration), and hooks: programs settings.toml
# names to run when something happens in the viewer.

settings-integration-ipc = Controle remoto e eventos
settings-integration-ipc-help = Outros programas neste computador, executados com seu usuário, podem controlar o { -app-name } e saber o que acontece nele por um socket privado (specular ipc). Nada fica escutando na rede.

## The hooks panel under "Remote control and events".
hooks-panel-title = Ganchos
hooks-panel-intro = Programas que o settings.toml executa quando algo acontece. Cada um pergunta uma vez antes da primeira execução.
hooks-panel-none = Nenhum gancho. Adicione-os como [[hooks]] no settings.toml.
hooks-panel-keywords = ganchos hooks eventos automação scripts programas
# $events: the event names, joined ("marked, exported").
hooks-panel-on = Quando { $events }
hooks-panel-untrusted = Pergunta antes da primeira execução
hooks-panel-allowed = Permitido
hooks-panel-remove = Remover…
hooks-panel-unused = Não usado

## Asking before a hook's first run (and again when its program or arguments change).
# $name: the hook's name; $program: the program; $events: what sets it off (hook-event-*,
# joined with hook-or).
hook-consent-title = Executar “{ $name }” ({ $program }) quando { $events }?
hook-consent-detail = Ele inicia este programa com os nomes das fotos: { $args }
hook-consent-trust = Permita só programas em que você confia. Ele pergunta de novo se o gancho mudar.
hook-consent-allow = Permitir
hook-consent-not-now = Agora não
hook-or = { " " }ou{ " " }
hook-event-opened = uma foto é aberta
hook-event-marked = fotos são marcadas
hook-event-keywords = palavras-chave mudam
hook-event-exported = fotos são exportadas
hook-event-imported = fotos são importadas
hook-event-captured = uma câmera envia uma foto
hook-event-develop-saved = uma edição de Revelar é salva
hook-event-any = qualquer coisa acontece

## Removing a hook (Settings › Integration › Hooks › Remove…).
hook-remove-title = Remover o gancho “{ $name }”?
hook-remove-detail = Ele é retirado do settings.toml. O programa e seus arquivos continuam no disco.
hook-remove-ok = Remover
hook-removed = Gancho “{ $name }” removido.
hook-settings-unwritable = Não foi possível alterar o settings.toml: { $reason }

## A hook that did not work (a quiet note; the log has the details).
hook-failed = O gancho “{ $name }” não funcionou: { $reason }
hook-failed-exit = terminou com o código { $code }
hook-failed-timeout = foi interrompido após { $seconds } s
hook-failed-start = não pôde iniciar ({ $reason })
hook-dropped = O gancho “{ $name }” está ocupado: { $count ->
    [one] um evento foi ignorado.
   *[other] { $count } eventos foram ignorados.
}

## An export started by another program (specular ipc export): nobody was asked.
export-recipes-remote-exists = { $count ->
    [one] existe um arquivo que seria substituído: nada foi exportado
   *[other] existem { $count } arquivos que seriam substituídos: nada foi exportado
}
export-recipes-remote-skipped = { $file } existe
