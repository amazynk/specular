# machine-drafted, needs native review
# Remote control and events (Settings → Integration), and hooks: programs settings.toml
# names to run when something happens in the viewer.

settings-integration-ipc = Controlo remoto e eventos
settings-integration-ipc-help = Outros programas neste computador, executados como o seu utilizador, podem controlar o { -app-name } e saber o que nele acontece através de um socket privado (specular ipc). Nada fica à escuta na rede.

## The hooks panel under "Remote control and events".
hooks-panel-title = Ganchos
hooks-panel-intro = Programas que o settings.toml executa quando algo acontece. Cada um pergunta uma vez antes da primeira execução.
hooks-panel-none = Nenhum gancho. Adicione-os como [[hooks]] no settings.toml.
hooks-panel-keywords = ganchos hooks eventos automatização scripts programas
# $events: the event names, joined ("marked, exported").
hooks-panel-on = Quando { $events }
hooks-panel-untrusted = Pergunta antes da primeira execução
hooks-panel-allowed = Permitido
hooks-panel-remove = Remover…
hooks-panel-unused = Não utilizado

## Asking before a hook's first run (and again when its program or arguments change).
# $name: the hook's name; $program: the program; $events: what sets it off (hook-event-*,
# joined with hook-or).
hook-consent-title = Executar “{ $name }” ({ $program }) quando { $events }?
hook-consent-detail = Inicia este programa com os nomes das fotografias: { $args }
hook-consent-trust = Permita apenas programas em que confia. Volta a perguntar se o gancho mudar.
hook-consent-allow = Permitir
hook-consent-not-now = Agora não
hook-or = { " " }ou{ " " }
hook-event-opened = uma fotografia é aberta
hook-event-marked = fotografias são marcadas
hook-event-keywords = palavras-chave mudam
hook-event-exported = fotografias são exportadas
hook-event-imported = fotografias são importadas
hook-event-captured = uma câmara envia uma fotografia
hook-event-develop-saved = uma edição de Revelar é guardada
hook-event-any = algo acontece

## Removing a hook (Settings › Integration › Hooks › Remove…).
hook-remove-title = Remover o gancho “{ $name }”?
hook-remove-detail = É retirado do settings.toml. O programa e os seus ficheiros continuam no disco.
hook-remove-ok = Remover
hook-removed = Gancho “{ $name }” removido.
hook-settings-unwritable = Não foi possível alterar o settings.toml: { $reason }

## A hook that did not work (a quiet note; the log has the details).
hook-failed = O gancho “{ $name }” não funcionou: { $reason }
hook-failed-exit = terminou com o código { $code }
hook-failed-timeout = foi parado após { $seconds } s
hook-failed-start = não foi possível iniciar ({ $reason })
hook-dropped = O gancho “{ $name }” está ocupado: { $count ->
    [one] um evento foi ignorado.
   *[other] { $count } eventos foram ignorados.
}

## An export started by another program (specular ipc export): nobody was asked.
export-recipes-remote-exists = { $count ->
    [one] existe um ficheiro que seria substituído: nada foi exportado
   *[other] existem { $count } ficheiros que seriam substituídos: nada foi exportado
}
export-recipes-remote-skipped = { $file } existe
