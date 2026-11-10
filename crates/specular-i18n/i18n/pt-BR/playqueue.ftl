# The play queue: what plays next, repeat and shuffle, playlists (M3U, PLS, XSPF), the
# queue panel and the sound transport's buttons.

# machine-drafted, needs native review
cheatsheet-video-queue = Fila, repetir e aleatório
# machine-drafted, needs native review
palette-next-track = Próximo item
# machine-drafted, needs native review
palette-next-track-help = Reproduzir o próximo item da fila ou, se não houver, o próximo arquivo do mesmo tipo na pasta.
# machine-drafted, needs native review
palette-prev-track = Item anterior
# machine-drafted, needs native review
palette-prev-track-help = Reproduzir o item anterior. Depois de 3 segundos, recomeçar este.
# machine-drafted, needs native review
palette-cycle-repeat = Repetir
# machine-drafted, needs native review
palette-cycle-repeat-help = Repetir desligado, tudo ou um.
# machine-drafted, needs native review
palette-shuffle = Aleatório
# machine-drafted, needs native review
palette-shuffle-help = Reproduzir em ordem aleatória, cada item uma vez por rodada.
# machine-drafted, needs native review
palette-play-queue = Fila de reprodução
# machine-drafted, needs native review
palette-play-queue-help = Mostrar o que toca em seguida: reordenar, remover, salvar.
# machine-drafted, needs native review
palette-queue-play-next = Tocar em seguida
# machine-drafted, needs native review
palette-queue-play-next-help = Reproduzir o arquivo (na galeria, a seleção) depois do item atual.
# machine-drafted, needs native review
palette-queue-add = Adicionar à fila
# machine-drafted, needs native review
palette-queue-add-help = Adicionar o arquivo (na galeria, a seleção) ao fim da fila.
# machine-drafted, needs native review
palette-save-queue = Salvar a fila como playlist…
# machine-drafted, needs native review
palette-save-queue-help = Salvar a fila como playlist M3U8 ou XSPF.
# machine-drafted, needs native review
transport-prev-track = Item anterior
# machine-drafted, needs native review
transport-next-track = Próximo item
# machine-drafted, needs native review
queue-title = Fila de reprodução
# machine-drafted, needs native review
queue-summary = { $count ->
    [one] 1 item
   *[other] { $count } itens
}
# machine-drafted, needs native review
queue-summary-time = { $count ->
    [one] 1 item
   *[other] { $count } itens
} · { $time }
# machine-drafted, needs native review
queue-empty = A fila está vazia.
    Adicione arquivos de som e vídeos com Tocar em seguida ou Adicionar à fila, ou abra uma playlist.
# machine-drafted, needs native review
queue-empty-short = A fila está vazia.
# machine-drafted, needs native review
queue-playing = Tocando
# machine-drafted, needs native review
queue-repeat = Repetir
# machine-drafted, needs native review
queue-repeat-off = Repetir: não
# machine-drafted, needs native review
queue-repeat-all = Repetir: tudo
# machine-drafted, needs native review
queue-repeat-one = Repetir: um
# machine-drafted, needs native review
queue-shuffle = Aleatório
# machine-drafted, needs native review
queue-shuffle-on = Aleatório: sim
# machine-drafted, needs native review
queue-shuffle-off = Aleatório: não
# machine-drafted, needs native review
queue-save = Salvar como playlist…
# machine-drafted, needs native review
queue-save-tip = Salvar a fila como playlist M3U8 ou XSPF
# machine-drafted, needs native review
queue-clear = Limpar…
# machine-drafted, needs native review
queue-clear-tip = Limpar a fila. Os arquivos continuam no disco.
# machine-drafted, needs native review
queue-remove = Remover da fila (o arquivo continua no disco)
# machine-drafted, needs native review
queue-network-item = Item de rede, não reproduzido: { $address }
# machine-drafted, needs native review
queue-artist-title = { $artist } – { $title }
# machine-drafted, needs native review
queue-at-end = Não há item seguinte.
# machine-drafted, needs native review
queue-at-start = Não há item anterior.
# machine-drafted, needs native review
queue-missing = { $name } não existe mais. Ignorado.
# machine-drafted, needs native review
queue-network-not-played = Os itens de rede ainda não são reproduzidos.
# machine-drafted, needs native review
queue-nothing-to-add = Nada a adicionar. Só arquivos de som, vídeos e playlists entram na fila.
# machine-drafted, needs native review
queue-added = { $count ->
    [one] 1 item adicionado à fila.
   *[other] { $count } itens adicionados à fila.
}
# machine-drafted, needs native review
queue-added-next = { $count ->
    [one] 1 item toca em seguida.
   *[other] { $count } itens tocam em seguida.
}
# machine-drafted, needs native review
queue-playlist-opened = { $name }: { $count ->
    [one] 1 item na fila.
   *[other] { $count } itens na fila.
}
# machine-drafted, needs native review
queue-playlist-nothing = Nada em { $name } pode ser reproduzido aqui. A fila mostra o que contém.
# machine-drafted, needs native review
queue-playlist-failed = Não é possível abrir { $name }: { $reason }
# machine-drafted, needs native review
queue-playlist-unreadable = não é possível ler o arquivo.
# machine-drafted, needs native review
queue-playlist-damaged = não é uma playlist que o Specular consiga ler.
# machine-drafted, needs native review
queue-clear-ask = { $count ->
    [one] Limpar a fila (1 item)?
   *[other] Limpar a fila ({ $count } itens)?
}
# machine-drafted, needs native review
queue-clear-detail = Só a lista é limpa. Os arquivos continuam no disco.
# machine-drafted, needs native review
queue-clear-ok = Limpar fila
# machine-drafted, needs native review
queue-cleared = Fila limpa.
# machine-drafted, needs native review
queue-save-title = Salvar a fila como playlist
# machine-drafted, needs native review
queue-default-name = Fila
# machine-drafted, needs native review
queue-format-m3u8 = Playlist M3U8
# machine-drafted, needs native review
queue-format-xspf = Playlist XSPF
# machine-drafted, needs native review
queue-saved = { $name } salvo.
# machine-drafted, needs native review
queue-save-failed = Não é possível salvar { $name }: { $reason }
# machine-drafted, needs native review
settings-viewing-sound-play-next = Reproduzir o próximo arquivo de som
# machine-drafted, needs native review
settings-viewing-sound-play-next-help = Quando um arquivo de som termina, toca o próximo da pasta.
# machine-drafted, needs native review
settings-viewing-play-repeat = Repetir
# machine-drafted, needs native review
settings-viewing-play-repeat-help = Para arquivos de som, vídeos e a fila.
# machine-drafted, needs native review
settings-viewing-play-repeat-off = Não
# machine-drafted, needs native review
settings-viewing-play-repeat-all = Tudo
# machine-drafted, needs native review
settings-viewing-play-repeat-one = Um
# machine-drafted, needs native review
settings-viewing-play-shuffle = Aleatório
# machine-drafted, needs native review
settings-viewing-play-shuffle-help = Reproduzir em ordem aleatória, cada item uma vez por rodada.
