# machine-drafted, needs native review
# Specular — Library (DESIGN §10).

palette-library = Biblioteca

## Top bar
library-title = Biblioteca
library-locations-button = Locais

## Locations sidebar
library-locations = Locais
library-all = Todos os locais
library-add = Adicionar local…
library-remove = Remover { $location } da biblioteca
library-offline = Off-line
library-location-pending = { $count } a indexar
library-a11y-location = { $location }, { $count ->
    [one] { $count } imagem
   *[other] { $count } imagens
}
library-a11y-location-offline = { $location }, Off-line, { $count ->
    [one] { $count } imagem
   *[other] { $count } imagens
}

## Indexing progress
library-walking = Imagens encontradas: { $count } · { $location }
# machine-drafted, needs native review
library-walking-count = Imagens encontradas: { $count }
library-reading = Indexando… { $done } de { $total }
library-held = A indexação pausa enquanto você navega
library-up-to-date = Atualizada
# machine-drafted, needs native review
library-stopped = Indexação interrompida
library-stopped-pending = Indexação interrompida · { $count } a indexar
library-resume = Retomar
# machine-drafted, needs native review
# The indexer stopped by itself (a database error, a bug) or stopped moving; $reason says why.
library-halted = Indexação interrompida: { $reason }
library-reason-stalled = nada se moveu por { $minutes ->
    [one] um minuto
   *[other] { $minutes } minutos
}. Um arquivo ou unidade pode não estar respondendo.
library-retry = Tentar novamente
library-retry-description = Inicia a indexação de novo. O que já foi indexado continua pesquisável.
library-save-diagnostics = Salvar pacote de diagnóstico…
library-stop-description = Interrompe a indexação. O que já foi indexado continua pesquisável; Retomar ou Verificar novamente (F5) continua o trabalho.
library-resume-description = Percorre todos os locais de novo e indexa o que falta.
library-opening = Abrindo a biblioteca…
library-searching = Pesquisando…

## Empty and error states
library-empty = Nenhum local ainda. Adicione uma pasta: as imagens dela e das subpastas aparecem aqui em uma única grade.
library-empty-offline = Estes locais estão off-line. As imagens voltam a aparecer quando eles voltarem.
library-indexing = Procurando imagens…
# machine-drafted, needs native review
library-remove-confirm = Remover “{ $location }” da biblioteca?
library-remove-confirm-detail = Suas fotos continuam no disco, com classificações, rótulos e palavras-chave. A biblioteca esquece o que indexou ali; se você adicionar a pasta de novo, ela é indexada outra vez.
library-remove-confirm-ok = Remover
library-indexing-in = Procurando imagens em { $location }… { $count } encontradas até agora
# machine-drafted, needs native review
library-empty-stopped = A indexação foi interrompida antes de estas imagens serem lidas. Retomar continua a partir daí.
library-unavailable = Não foi possível abrir a biblioteca: { $reason }
library-read-only = Esta biblioteca foi gravada por um Specular mais novo: é possível pesquisar, mas não atualizar.

## Screen readers
library-a11y-list = Biblioteca, { $count ->
    [one] { $count } imagem
   *[other] { $count } imagens
}
library-a11y-added = { $location } adicionado à biblioteca
library-a11y-removed = { $location } removido da biblioteca

## Adding the home folder
# machine-drafted, needs native review
library-home-confirm = Adicionar tudo em “{ $folder }”?
library-home-confirm-detail = Ela contém arquivos de programas, downloads e caches, não só fotos. A indexação demora mais e a biblioteca se enche de ícones e outras imagens.
library-home-confirm-pictures = Sua pasta Imagens contém apenas suas imagens.
library-home-confirm-use-pictures = Adicionar Imagens
library-home-confirm-keep = Adicionar pasta pessoal

## Folder watches
# machine-drafted, needs native review
library-unwatched = Algumas pastas não são monitoradas
library-unwatched-detail = { $minutes ->
    [one] As alterações aparecem em até um minuto
   *[other] As alterações aparecem em até { $minutes } min
}
library-unwatched-description = O Specular ficou sem monitoramentos de pasta. Abre a ajuda sobre como aumentar o limite.

## Settings → Library
# machine-drafted, needs native review
settings-section-library = Biblioteca
settings-library-skip-folders = Ignorar pastas chamadas
settings-library-skip-folders-help = A biblioteca não procura em pastas com estes nomes, separados por vírgulas. Pastas ocultas são sempre ignoradas.
settings-library-skip-marked = Ignorar pastas que contêm
settings-library-skip-marked-help = Nem em pastas que contêm um destes: .git para repositórios de código, pyvenv.cfg para ambientes Python, CACHEDIR.TAG para caches. A pasta de um local nunca é ignorada.
settings-library-show-timeline = Linha do tempo
settings-library-show-timeline-help = Mostra quantas fotos há em cada ano, mês ou dia, acima da grade da biblioteca.
