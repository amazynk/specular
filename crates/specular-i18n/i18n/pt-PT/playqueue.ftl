# The play queue: what plays next, repeat and shuffle, playlists (M3U, PLS, XSPF), the
# queue panel and the sound transport's buttons.

# machine-drafted, needs native review
cheatsheet-video-queue = Fila, repetir e aleatório
# machine-drafted, needs native review
palette-next-track = Item seguinte
# machine-drafted, needs native review
palette-next-track-help = Reproduzir o item seguinte da fila ou, se não houver, o ficheiro seguinte do mesmo tipo na pasta.
# machine-drafted, needs native review
palette-prev-track = Item anterior
# machine-drafted, needs native review
palette-prev-track-help = Reproduzir o item anterior. Passados 3 segundos, recomeçar este.
# machine-drafted, needs native review
palette-cycle-repeat = Repetir
# machine-drafted, needs native review
palette-cycle-repeat-help = Repetir desligado, tudo ou um.
# machine-drafted, needs native review
palette-shuffle = Aleatório
# machine-drafted, needs native review
palette-shuffle-help = Reproduzir por ordem aleatória, cada item uma vez por volta.
# machine-drafted, needs native review
palette-play-queue = Fila de reprodução
# machine-drafted, needs native review
palette-play-queue-help = Mostrar o que toca a seguir: reordenar, remover, guardar.
# machine-drafted, needs native review
palette-queue-play-next = Reproduzir a seguir
# machine-drafted, needs native review
palette-queue-play-next-help = Reproduzir o ficheiro (na galeria, a seleção) depois do item atual.
# machine-drafted, needs native review
palette-queue-add = Adicionar à fila
# machine-drafted, needs native review
palette-queue-add-help = Adicionar o ficheiro (na galeria, a seleção) ao fim da fila.
# machine-drafted, needs native review
palette-save-queue = Guardar a fila como lista…
# machine-drafted, needs native review
palette-save-queue-help = Guardar a fila como lista de reprodução M3U8 ou XSPF.
# machine-drafted, needs native review
transport-prev-track = Item anterior
# machine-drafted, needs native review
transport-next-track = Item seguinte
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
    Adicione ficheiros de som e vídeos com Reproduzir a seguir ou Adicionar à fila, ou abra uma lista de reprodução.
# machine-drafted, needs native review
queue-empty-short = A fila está vazia.
# machine-drafted, needs native review
queue-playing = A tocar
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
queue-save = Guardar como lista…
# machine-drafted, needs native review
queue-save-tip = Guardar a fila como lista de reprodução M3U8 ou XSPF
# machine-drafted, needs native review
queue-clear = Limpar…
# machine-drafted, needs native review
queue-clear-tip = Limpar a fila. Os ficheiros ficam no disco.
# machine-drafted, needs native review
queue-remove = Remover da fila (o ficheiro fica no disco)
# machine-drafted, needs native review
queue-network-item = Item de rede, não reproduzido: { $address }
# machine-drafted, needs native review
queue-artist-title = { $artist } – { $title }
# machine-drafted, needs native review
queue-at-end = Não há item seguinte.
# machine-drafted, needs native review
queue-at-start = Não há item anterior.
# machine-drafted, needs native review
queue-missing = { $name } já não existe. Ignorado.
# machine-drafted, needs native review
queue-network-not-played = Os itens de rede ainda não são reproduzidos.
# machine-drafted, needs native review
queue-nothing-to-add = Nada a adicionar. Só ficheiros de som, vídeos e listas de reprodução entram na fila.
# machine-drafted, needs native review
queue-added = { $count ->
    [one] 1 item adicionado à fila.
   *[other] { $count } itens adicionados à fila.
}
# machine-drafted, needs native review
queue-added-next = { $count ->
    [one] 1 item toca a seguir.
   *[other] { $count } itens tocam a seguir.
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
queue-playlist-unreadable = não é possível ler o ficheiro.
# machine-drafted, needs native review
queue-playlist-damaged = não é uma lista de reprodução que o Specular saiba ler.
# machine-drafted, needs native review
queue-clear-ask = { $count ->
    [one] Limpar a fila (1 item)?
   *[other] Limpar a fila ({ $count } itens)?
}
# machine-drafted, needs native review
queue-clear-detail = Só a lista é limpa. Os ficheiros ficam no disco.
# machine-drafted, needs native review
queue-clear-ok = Limpar fila
# machine-drafted, needs native review
queue-cleared = Fila limpa.
# machine-drafted, needs native review
queue-save-title = Guardar a fila como lista
# machine-drafted, needs native review
queue-default-name = Fila
# machine-drafted, needs native review
queue-format-m3u8 = Lista M3U8
# machine-drafted, needs native review
queue-format-xspf = Lista XSPF
# machine-drafted, needs native review
queue-saved = { $name } guardado.
# machine-drafted, needs native review
queue-save-failed = Não é possível guardar { $name }: { $reason }
# machine-drafted, needs native review
settings-viewing-sound-play-next = Reproduzir o ficheiro de som seguinte
# machine-drafted, needs native review
settings-viewing-sound-play-next-help = Quando um ficheiro de som termina, é reproduzido o seguinte da pasta.
# machine-drafted, needs native review
settings-viewing-play-repeat = Repetir
# machine-drafted, needs native review
settings-viewing-play-repeat-help = Para ficheiros de som, vídeos e a fila.
# machine-drafted, needs native review
settings-viewing-play-repeat-off = Não
# machine-drafted, needs native review
settings-viewing-play-repeat-all = Tudo
# machine-drafted, needs native review
settings-viewing-play-repeat-one = Um
# machine-drafted, needs native review
settings-viewing-play-shuffle = Aleatório
# machine-drafted, needs native review
settings-viewing-play-shuffle-help = Reproduzir por ordem aleatória, cada item uma vez por volta.
