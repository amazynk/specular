# machine-drafted, needs native review
# Specular — Library (DESIGN §10).

palette-library = Biblioteca

## Top bar
library-title = Biblioteca
library-locations-button = Localizações

## Locations sidebar
library-locations = Localizações
library-all = Todas as localizações
library-add = Adicionar localização…
library-remove = Remover { $location } da biblioteca
library-offline = Offline
library-location-pending = { $count } por indexar
library-a11y-location = { $location }, { $count ->
    [one] { $count } imagem
   *[other] { $count } imagens
}
library-a11y-location-offline = { $location }, Offline, { $count ->
    [one] { $count } imagem
   *[other] { $count } imagens
}

## Indexing progress
library-walking = Imagens encontradas: { $count } · { $location }
# machine-drafted, needs native review
library-walking-count = Imagens encontradas: { $count }
library-reading = A indexar… { $done } de { $total }
library-held = A indexação pausa enquanto navega
library-up-to-date = Atualizada
# machine-drafted, needs native review
library-stopped = Indexação interrompida
library-stopped-pending = Indexação interrompida · { $count } por indexar
library-resume = Retomar
# machine-drafted, needs native review
# The indexer stopped by itself (a database error, a bug) or stopped moving; $reason says why.
library-halted = Indexação interrompida: { $reason }
library-reason-stalled = nada se moveu durante { $minutes ->
    [one] um minuto
   *[other] { $minutes } minutos
}. Um ficheiro ou unidade pode não estar a responder.
library-retry = Tentar novamente
library-retry-description = Inicia a indexação de novo. O que já foi indexado continua pesquisável.
library-save-diagnostics = Guardar pacote de diagnóstico…
library-stop-description = Interrompe a indexação. O que já foi indexado continua pesquisável; Retomar ou Analisar novamente (F5) prossegue.
library-resume-description = Percorre novamente todas as localizações e indexa o que falta.
library-opening = A abrir a biblioteca…
library-searching = A pesquisar…

## Empty and error states
library-empty = Ainda não há localizações. Adicione uma pasta: as imagens dela e das subpastas aparecem aqui numa só grelha.
library-empty-offline = Estas localizações estão offline. As imagens voltam a aparecer quando regressarem.
library-indexing = A procurar imagens…
# machine-drafted, needs native review
library-remove-confirm = Remover «{ $location }» da biblioteca?
library-remove-confirm-detail = As suas fotografias ficam no disco, com classificações, etiquetas e palavras-chave. A biblioteca esquece o que indexou ali; se voltar a adicionar a pasta, ela é indexada de novo.
library-remove-confirm-ok = Remover
library-indexing-in = A procurar imagens em { $location }… { $count } encontradas até agora
# machine-drafted, needs native review
library-empty-stopped = A indexação foi interrompida antes de estas imagens serem lidas. Retomar prossegue a partir daí.
library-unavailable = Não foi possível abrir a biblioteca: { $reason }
library-read-only = Esta biblioteca foi escrita por um Specular mais recente: pode ser pesquisada, mas não atualizada.

## Screen readers
library-a11y-list = Biblioteca, { $count ->
    [one] { $count } imagem
   *[other] { $count } imagens
}
library-a11y-added = { $location } adicionada à biblioteca
library-a11y-removed = { $location } removida da biblioteca

## Adding the home folder
# machine-drafted, needs native review
library-home-confirm = Adicionar tudo em «{ $folder }»?
library-home-confirm-detail = Contém ficheiros de programas, transferências e caches, não só fotografias. A indexação demora mais e a biblioteca enche-se de ícones e outras imagens.
library-home-confirm-pictures = A sua pasta Imagens contém apenas as suas imagens.
library-home-confirm-use-pictures = Adicionar Imagens
library-home-confirm-keep = Adicionar pasta pessoal

## Folder watches
# machine-drafted, needs native review
library-unwatched = Algumas pastas não são vigiadas
library-unwatched-detail = { $minutes ->
    [one] As alterações aparecem em menos de um minuto
   *[other] As alterações aparecem em { $minutes } min
}
library-unwatched-description = O Specular ficou sem vigilâncias de pastas. Abre a ajuda sobre como aumentar o limite.

## Settings → Library
# machine-drafted, needs native review
settings-section-library = Biblioteca
settings-library-skip-folders = Ignorar pastas com o nome
settings-library-skip-folders-help = A biblioteca não procura em pastas com estes nomes, separados por vírgulas. As pastas ocultas são sempre ignoradas.
settings-library-skip-marked = Ignorar pastas que contenham
settings-library-skip-marked-help = Nem em pastas que contenham um destes: .git para repositórios de código, pyvenv.cfg para ambientes Python, CACHEDIR.TAG para caches. A pasta de uma localização nunca é ignorada.
settings-library-show-timeline = Cronologia
settings-library-show-timeline-help = Mostra quantas fotografias há em cada ano, mês ou dia, acima da grelha da biblioteca.
