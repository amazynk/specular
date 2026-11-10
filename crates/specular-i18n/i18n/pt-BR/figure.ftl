# Figures, key images and teaching files.

## Command palette
# machine-drafted, needs native review
palette-export-figure = Exportar figura…
# machine-drafted, needs native review
palette-export-figure-help = Salvar a visualização como figura com barra de escala, barra de cores e rótulos.
# machine-drafted, needs native review
palette-toggle-key-image = Marcar imagem-chave
# machine-drafted, needs native review
palette-toggle-key-image-help = Marcar esta imagem como imagem-chave, ou remover a marca.
# machine-drafted, needs native review
palette-copy-key-images = Copiar imagens-chave
# machine-drafted, needs native review
palette-copy-key-images-help = Copiar as imagens-chave como uma única imagem.
# machine-drafted, needs native review
palette-export-key-images = Exportar imagens-chave…
# machine-drafted, needs native review
palette-export-key-images-help = Salvar as imagens-chave como slides ou como uma pasta de PNGs.
# machine-drafted, needs native review
palette-export-teaching-file = Exportar caso de ensino…
# machine-drafted, needs native review
palette-export-teaching-file-help = Salvar as imagens-chave sem os dados do paciente, para ensino.

## The figure dialog
# machine-drafted, needs native review
figure-dialog-title = Exportar figura
# machine-drafted, needs native review
figure-format = Formato
# machine-drafted, needs native review
figure-format-png = PNG, 16 bits
# machine-drafted, needs native review
figure-format-tiff = TIFF, 16 bits
# machine-drafted, needs native review
figure-region = Região
# machine-drafted, needs native review
figure-region-visible = Área visível
# machine-drafted, needs native review
figure-region-whole = Imagem inteira
# machine-drafted, needs native review
figure-size = Tamanho
# machine-drafted, needs native review
figure-size-pixels = Pixels da imagem (1:1)
# $width is a width in pixels.
# machine-drafted, needs native review
figure-size-width = { $width } px de largura
# machine-drafted, needs native review
figure-scale-bar = Barra de escala
# machine-drafted, needs native review
figure-scale-bar-none = Sem calibração
# machine-drafted, needs native review
figure-colour-bar = Barra de cores
# machine-drafted, needs native review
figure-colour-bar-none = Sem mapa de cores nem janela
# machine-drafted, needs native review
figure-inset = Detalhe ampliado
# machine-drafted, needs native review
figure-inset-rectangle = Último retângulo
# machine-drafted, needs native review
figure-inset-none = Desenhe um retângulo primeiro
# machine-drafted, needs native review
figure-channels = Canais
# machine-drafted, needs native review
figure-channels-merged = Mesclados
# machine-drafted, needs native review
figure-channels-split = Separados e mesclados
# machine-drafted, needs native review
figure-channels-one = Um canal
# machine-drafted, needs native review
figure-measurements = Medições
# $count is the number of measurements.
# machine-drafted, needs native review
figure-measurements-on = { $count ->
    [one] Ativadas ({ $count })
    [many] Ativadas ({ $count })
   *[other] Ativadas ({ $count })
}
# machine-drafted, needs native review
figure-measurements-none = Nenhuma
# machine-drafted, needs native review
figure-title-row = Título
# machine-drafted, needs native review
figure-provenance = Proveniência
# machine-drafted, needs native review
figure-provenance-metadata = No arquivo
# machine-drafted, needs native review
figure-provenance-drawn = No arquivo e desenhada
# machine-drafted, needs native review
figure-file = Arquivo
# machine-drafted, needs native review
figure-choose = Escolher…
# machine-drafted, needs native review
figure-on = Ativado
# machine-drafted, needs native review
figure-off = Desativado
# $source is where the value comes from (a calibration, a colour map).
# machine-drafted, needs native review
figure-on-from = Ativado · { $source }
# machine-drafted, needs native review
figure-export = Exportar
# machine-drafted, needs native review
figure-working = Exportando…
# machine-drafted, needs native review
figure-not-full-res = Os pixels em resolução total não foram carregados. Dê zoom para { PERCENT(100) } ou escolha um tamanho.
# machine-drafted, needs native review
figure-channel-red = Vermelho
# machine-drafted, needs native review
figure-channel-green = Verde
# machine-drafted, needs native review
figure-channel-blue = Azul
# $map is a colour map's name; $lo and $hi the values at its ends; $unit their unit.
# machine-drafted, needs native review
figure-legend-map = { $map }, de { $lo } a { $hi } { $unit }
# $lo and $hi are the window's ends; $unit their unit (HU).
# machine-drafted, needs native review
figure-legend-window = Janela de { $lo } a { $hi } { $unit }
# $preset is a channel-colour preset's id.
# machine-drafted, needs native review
figure-display-channels = Cores dos canais { $preset }
# $channel is the channel shown (red, green, blue, alpha, luma).
# machine-drafted, needs native review
figure-display-channel = Canal { $channel }
# machine-drafted, needs native review
figure-display-hilo = Marcas HiLo
# machine-drafted, needs native review
figure-display-as-stored = Como armazenada
# $value is the calibration (source and spacing).
# machine-drafted, needs native review
figure-caption-calibration = Calibração: { $value }
# $value is the display settings.
# machine-drafted, needs native review
figure-caption-display = Exibição: { $value }
# machine-drafted, needs native review
figure-not-image = As figuras são criadas a partir de uma imagem no visualizador.
# machine-drafted, needs native review
figure-not-ready = A imagem ainda está carregando.
# $width and $height are in pixels.
# machine-drafted, needs native review
figure-too-large = Grande demais: { $width } × { $height } px. Escolha a área visível ou um tamanho.

## Key images
# machine-drafted, needs native review
figure-keys-title = Exportar imagens-chave
# machine-drafted, needs native review
figure-teaching-title = Exportar caso de ensino
# machine-drafted, needs native review
figure-keys-images = Imagens
# $count is the number of key images.
# machine-drafted, needs native review
figure-keys-count = { $count ->
    [one] { $count } imagem-chave
    [many] { $count } imagens-chave
   *[other] { $count } imagens-chave
}
# machine-drafted, needs native review
figure-keys-pptx = Slides (PPTX)
# machine-drafted, needs native review
figure-keys-folder = Pasta de PNGs
# machine-drafted, needs native review
figure-keys-preset = Predefinição
# machine-drafted, needs native review
figure-keys-standard = Padrão
# machine-drafted, needs native review
figure-keys-teaching = Caso de ensino
# machine-drafted, needs native review
figure-keys-overlays-on = Desenhadas e nas notas
# machine-drafted, needs native review
figure-keys-masks = Texto gravado na imagem
# machine-drafted, needs native review
figure-keys-masks-auto = Tarjado em preto
# machine-drafted, needs native review
figure-keys-notes = Notas do caso
# machine-drafted, needs native review
figure-keys-destination = Salvar em
# machine-drafted, needs native review
figure-keys-empty = Ainda não há imagens-chave. Marque primeiro uma imagem como imagem-chave.
# machine-drafted, needs native review
figure-keys-teaching-note = Os dados do paciente são omitidos e o texto gravado na imagem é tarjado em preto. Confira cada imagem antes de compartilhar.
# machine-drafted, needs native review
figure-keys-none-left = Nenhuma imagem pôde ser exportada sem dados do paciente.
# machine-drafted, needs native review
figure-keys-copied = Imagens-chave copiadas
# $count is how many key images there are now.
# machine-drafted, needs native review
figure-key-marked = { $count ->
    [one] Marcada como imagem-chave ({ $count })
    [many] Marcada como imagem-chave ({ $count })
   *[other] Marcada como imagem-chave ({ $count })
}
# $count is how many key images are left.
# machine-drafted, needs native review
figure-key-unmarked = { $count ->
    [one] Não é mais imagem-chave (resta { $count })
    [many] Não é mais imagem-chave (restam { $count })
   *[other] Não é mais imagem-chave (restam { $count })
}
# $n is a frame or page number.
# machine-drafted, needs native review
figure-key-frame = Imagem { $n }
# $n is the key image's number in the set.
# machine-drafted, needs native review
figure-key-n = Imagem-chave { $n }

## Results
# $name is the file or folder written.
# machine-drafted, needs native review
figure-saved = { $name } salvo
# $name is the file or folder written; $count how many images were left out.
# machine-drafted, needs native review
figure-saved-skipped = { $count ->
    [one] { $name } salvo. { $count } imagem foi deixada de fora: o texto gravado nela não foi encontrado.
    [many] { $name } salvo. { $count } imagens foram deixadas de fora: o texto gravado nelas não foi encontrado.
   *[other] { $name } salvo. { $count } imagens foram deixadas de fora: o texto gravado nelas não foi encontrado.
}
# $reason says why.
# machine-drafted, needs native review
figure-failed = Não foi possível exportar: { $reason }
# $name is a file that is already in the folder.
# machine-drafted, needs native review
figure-folder-taken = { $name } já está nessa pasta. Escolha uma pasta vazia.

## The viewer
# machine-drafted, needs native review
figure-key-badge = Imagem-chave
