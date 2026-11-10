# Specular — Galerias para clientes: uma pasta HTML estática de fotos, e a leitura de volta da escolha do cliente.

# machine-drafted, needs native review

palette-client-gallery = Galeria para cliente…
palette-client-gallery-help = Salvar as fotos como uma pasta de página web para um cliente, com uma escolha que ele pode devolver.
palette-import-client-picks = Importar escolha do cliente…
palette-import-client-picks-help = Marcar as fotos que um cliente escolheu na galeria dele.

webgallery-heading = Galeria para cliente
webgallery-subtitle = { $count ->
    [one] Uma foto
   *[other] { $count } fotos
}
webgallery-title-label = Título
webgallery-title-default = Provas
webgallery-watermark-label = Texto da marca d’água (vazio: nenhuma)
webgallery-source = Fotos
webgallery-source-current = Esta foto
webgallery-source-selection = Seleção
webgallery-source-album = Álbum “{ $name }”
webgallery-source-search = Resultados da busca
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
webgallery-captions = Abaixo de cada foto
webgallery-captions-off = Nada
webgallery-captions-name = Nome do arquivo
webgallery-captions-title = Título
webgallery-captions-all = Nome do arquivo, título e legenda
webgallery-exif = Mostrar câmera e exposição
webgallery-location = Manter a localização (GPS) nas fotos
webgallery-note-private = Uma galeria estática não pode ser protegida por senha.
webgallery-note-share = Qualquer pessoa com a pasta ou o endereço dela vê todas as fotos. Compartilhe o link só com seu cliente, ou use o controle de acesso da sua hospedagem.
webgallery-note-picks = Os clientes escolhem com um coração. A escolha fica no navegador deles até que enviem a lista para você.
webgallery-write = Criar galeria…
webgallery-busy = Gravando { $done } de { $total }…
webgallery-stop = Parar
webgallery-done = { $count ->
    [one] Galeria criada: uma foto em { $folder }
   *[other] Galeria criada: { $count } fotos em { $folder }
}
webgallery-done-failed = { $failed ->
    [one] Não foi possível gravar uma foto.
   *[other] Não foi possível gravar { $failed } fotos.
}
webgallery-stopped = Galeria interrompida. Nada foi mantido.
webgallery-failed = Galeria não criada: { $reason }
webgallery-nothing = Nenhuma foto pôde ser gravada.

webgallery-page-count = { $count ->
    [one] Uma foto
   *[other] { $count } fotos
}
webgallery-page-hint = Toque no coração das fotos que você quer. Sua escolha fica neste navegador até você enviá-la.
webgallery-page-picked = Escolhidas: {"{"}n{"}"}
webgallery-page-pick = Escolher
webgallery-page-copy = Copiar minha escolha
webgallery-page-copied = Copiada. Cole a lista em uma mensagem.
webgallery-page-copy-by-hand = Copie a lista selecionada e cole-a em uma mensagem.
webgallery-page-download-txt = Baixar picks.txt
webgallery-page-download-json = Baixar picks.json
webgallery-page-clear = Limpar escolha
webgallery-page-clear-ask = Limpar toda a sua escolha neste navegador?
webgallery-page-no-storage = Este navegador não vai guardar sua escolha. Envie-a antes de fechar a página.
webgallery-page-noscript = Para escolher é preciso JavaScript. As fotos abrem mesmo assim.
webgallery-page-previous = Anterior
webgallery-page-next = Próxima
webgallery-page-close = Fechar
webgallery-page-position = {"{"}i{"}"} de {"{"}n{"}"}
webgallery-page-viewer = Visualizador de fotos
webgallery-page-picks = Sua escolha

clientpicks-heading = Importar escolha do cliente
clientpicks-subtitle = { $count ->
    [one] Comparada com uma foto exibida
   *[other] Comparada com as { $count } fotos exibidas
}
clientpicks-paste-label = Nomes (cole a lista do cliente)
clientpicks-choose = Escolher arquivo da escolha…
clientpicks-from-file = De { $name }
clientpicks-apply-as = Marcá-las
clientpicks-as-pick = Como escolhidas
clientpicks-as-stars = { $stars ->
    [one] Uma estrela
   *[other] { $stars } estrelas
}
clientpicks-as-label = Rótulo { $label }
clientpicks-empty = Cole a lista que o cliente enviou, ou escolha o picks.txt ou picks.json dele.
clientpicks-matched = { $matched } de { $total } nomes correspondem a fotos aqui.
clientpicks-missing = Não encontrados: { $names }
clientpicks-ambiguous = Em mais de uma pasta, deixados como estão: { $names }
clientpicks-more = { $names } e mais { $more }
clientpicks-apply = Aplicar
clientpicks-applied = { $count ->
    [one] Uma foto marcada.
   *[other] { $count } fotos marcadas.
}
clientpicks-unchanged = Essas fotos já estavam marcadas.
clientpicks-read-failed = Não é possível ler { $name }: { $reason }
