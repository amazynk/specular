# Specular — Galerias para clientes: uma pasta HTML estática de fotografias, e a leitura da escolha do cliente.

# machine-drafted, needs native review

palette-client-gallery = Galeria para cliente…
palette-client-gallery-help = Guardar as fotografias como uma pasta de página web para um cliente, com uma escolha que ele pode devolver.
palette-import-client-picks = Importar escolha do cliente…
palette-import-client-picks-help = Marcar as fotografias que um cliente escolheu na sua galeria.

webgallery-heading = Galeria para cliente
webgallery-subtitle = { $count ->
    [one] Uma fotografia
   *[other] { $count } fotografias
}
webgallery-title-label = Título
webgallery-title-default = Provas
webgallery-watermark-label = Texto da marca de água (vazio: nenhuma)
webgallery-source = Fotografias
webgallery-source-current = Esta fotografia
webgallery-source-selection = Seleção
webgallery-source-album = Álbum «{ $name }»
webgallery-source-search = Resultados da pesquisa
webgallery-source-folder = Esta pasta
webgallery-source-picks = Escolhidas
webgallery-source-value = { $what } ({ $count })
webgallery-theme = Tema
webgallery-theme-light = Claro
webgallery-theme-dark = Escuro
webgallery-theme-minimal = Mínimo
webgallery-size = Lado maior
webgallery-size-value = { $px } px
webgallery-format = Formato
webgallery-captions = Por baixo de cada fotografia
webgallery-captions-off = Nada
webgallery-captions-name = Nome do ficheiro
webgallery-captions-title = Título
webgallery-captions-all = Nome do ficheiro, título e legenda
webgallery-exif = Mostrar câmara e exposição
webgallery-location = Manter a localização (GPS) nas fotografias
webgallery-note-private = Uma galeria estática não pode ser protegida por palavra-passe.
webgallery-note-share = Quem tiver a pasta ou o seu endereço vê todas as fotografias. Partilhe a ligação apenas com o seu cliente, ou use o controlo de acesso do seu alojamento web.
webgallery-note-picks = Os clientes escolhem com um coração. A escolha fica no navegador deles até lhe enviarem a lista.
webgallery-write = Criar galeria…
webgallery-busy = A gravar { $done } de { $total }…
webgallery-stop = Parar
webgallery-done = { $count ->
    [one] Galeria criada: uma fotografia em { $folder }
   *[other] Galeria criada: { $count } fotografias em { $folder }
}
webgallery-done-failed = { $failed ->
    [one] Não foi possível gravar uma fotografia.
   *[other] Não foi possível gravar { $failed } fotografias.
}
webgallery-stopped = Galeria interrompida. Nada foi mantido.
webgallery-failed = Galeria não criada: { $reason }
webgallery-nothing = Não foi possível gravar nenhuma fotografia.

webgallery-page-count = { $count ->
    [one] Uma fotografia
   *[other] { $count } fotografias
}
webgallery-page-hint = Toque no coração das fotografias que quer. A sua escolha fica neste navegador até a enviar.
webgallery-page-picked = Escolhidas: {"{"}n{"}"}
webgallery-page-pick = Escolher
webgallery-page-copy = Copiar a minha escolha
webgallery-page-copied = Copiada. Cole a lista numa mensagem.
webgallery-page-copy-by-hand = Copie a lista selecionada e cole-a numa mensagem.
webgallery-page-download-txt = Transferir picks.txt
webgallery-page-download-json = Transferir picks.json
webgallery-page-clear = Limpar escolha
webgallery-page-clear-ask = Limpar toda a sua escolha neste navegador?
webgallery-page-no-storage = Este navegador não vai guardar a sua escolha. Envie-a antes de fechar a página.
webgallery-page-noscript = Para escolher é preciso JavaScript. As fotografias abrem na mesma.
webgallery-page-previous = Anterior
webgallery-page-next = Seguinte
webgallery-page-close = Fechar
webgallery-page-position = {"{"}i{"}"} de {"{"}n{"}"}
webgallery-page-viewer = Visualizador de fotografias
webgallery-page-picks = A sua escolha

clientpicks-heading = Importar escolha do cliente
clientpicks-subtitle = { $count ->
    [one] Comparada com uma fotografia apresentada
   *[other] Comparada com as { $count } fotografias apresentadas
}
clientpicks-paste-label = Nomes (cole a lista do cliente)
clientpicks-choose = Escolher ficheiro da escolha…
clientpicks-from-file = De { $name }
clientpicks-apply-as = Marcá-las
clientpicks-as-pick = Como escolhidas
clientpicks-as-stars = { $stars ->
    [one] Uma estrela
   *[other] { $stars } estrelas
}
clientpicks-as-label = Etiqueta { $label }
clientpicks-empty = Cole a lista que o cliente enviou, ou escolha o picks.txt ou picks.json dele.
clientpicks-matched = { $matched } de { $total } nomes correspondem a fotografias aqui.
clientpicks-missing = Não encontrados: { $names }
clientpicks-ambiguous = Em mais de uma pasta, deixados como estão: { $names }
clientpicks-more = { $names } e mais { $more }
clientpicks-apply = Aplicar
clientpicks-applied = { $count ->
    [one] Uma fotografia marcada.
   *[other] { $count } fotografias marcadas.
}
clientpicks-unchanged = Essas fotografias já estavam marcadas.
clientpicks-read-failed = Não é possível ler { $name }: { $reason }
