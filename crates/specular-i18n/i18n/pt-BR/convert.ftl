# Specular — Batch convert (DESIGN.md §6.5). machine-drafted, needs native review
# Every output is a new file; the sources are never changed.

palette-convert = Converter…

## Settings → Convert
settings-section-convert = Converter
settings-convert-format = Formato
settings-convert-format-help = O formato que Converter… grava. PNG e TIFF são sem perdas; WebP e JPEG XL podem ser de ambos os tipos.
settings-convert-format-jpeg = JPEG
settings-convert-format-png = PNG
settings-convert-format-webp = WebP
settings-convert-format-jxl = JPEG XL
settings-convert-format-tiff = TIFF
settings-convert-jpeg-quality = Qualidade JPEG
settings-convert-jpeg-quality-help = Valores mais altos mantêm mais detalhes em arquivos maiores. Com 90 é difícil distinguir do original.
settings-convert-jpeg-full-chroma = Cor em resolução total no JPEG
settings-convert-jpeg-full-chroma-help = Mantém a cor em resolução total (4:4:4) em vez de metade (4:2:0): bordas de cor mais nítidas, arquivos maiores.
settings-convert-png-effort = Compressão PNG
settings-convert-png-effort-help = Valores mais altos geram arquivos menores, mais devagar. 1 é o mais rápido.
settings-convert-jxl-effort = Esforço do JPEG XL
settings-convert-jxl-effort-help = Valores mais altos geram arquivos menores, mais devagar.
settings-convert-tiff-compression = Compressão TIFF
settings-convert-tiff-compression-help = Deflate é sem perdas e lido por qualquer leitor de TIFF.
settings-convert-tiff-compression-deflate = Deflate
settings-convert-tiff-compression-none = Nenhuma
settings-convert-depth = Profundidade de bits
settings-convert-depth-help = Automática grava 16 bits por amostra quando a imagem tem mais de 8 e o formato os suporta (PNG, TIFF, JPEG XL).
settings-convert-depth-auto = Automática
settings-convert-depth-8 = 8 bits
settings-convert-depth-16 = 16 bits
settings-convert-resize = Redimensionar
settings-convert-resize-help = As imagens são reduzidas em luz linear com um filtro de alta qualidade.
settings-convert-resize-none = Tamanho original
settings-convert-resize-fit = Caber em
settings-convert-resize-long-edge = Lado maior
settings-convert-resize-percent = Porcentagem
settings-convert-width = Largura
settings-convert-width-help = A largura máxima que uma imagem pode ter com Caber em.
settings-convert-height = Altura
settings-convert-height-help = A altura máxima que uma imagem pode ter com Caber em.
settings-convert-long-edge = Lado maior
settings-convert-long-edge-help = O comprimento do lado maior da imagem com Lado maior.
settings-convert-percent = Porcentagem
settings-convert-percent-help = O tamanho em relação ao original com Porcentagem.
settings-convert-upscale = Ampliar imagens pequenas
settings-convert-upscale-help = Desligado, as imagens já menores que o tamanho pedido mantêm o tamanho.
settings-convert-colour = Cor
settings-convert-colour-help = Manter grava o perfil de cor da imagem junto com ela. Converter para sRGB é indicado para a web e para dispositivos sem gerenciamento de cores.
settings-convert-colour-keep = Manter o perfil
settings-convert-colour-srgb = Converter para sRGB
settings-convert-hdr = Imagens HDR
settings-convert-hdr-help = O PNG e o JPEG XL mantêm o HDR como está; os outros formatos sempre passam por mapeamento de tons para SDR.
settings-convert-hdr-keep = Manter quando possível
settings-convert-hdr-tone-map = Mapear tons para SDR
settings-convert-metadata = Metadados
settings-convert-metadata-help = Exif e XMP, como a câmera, a data da foto e a localização. O perfil de cor é sempre mantido.
settings-convert-metadata-all = Manter tudo
settings-convert-metadata-no-location = Tudo menos a localização
settings-convert-metadata-strip = Remover
settings-convert-folder = Pasta de destino
settings-convert-folder-help = Para onde vão os arquivos convertidos. Se não for definida, eles vão para uma subpasta ao lado de cada imagem.
settings-convert-folder-unset = Subpasta ao lado das imagens
settings-convert-subfolder = Nome da subpasta
settings-convert-subfolder-help = A subpasta usada quando nenhuma pasta de destino está definida. Vazio: a própria pasta das imagens.
settings-convert-pattern = Nomes de arquivo
settings-convert-pattern-help = {"{"}name{"}"} é o nome original, {"{"}n:03{"}"} o número no lote, {"{"}date:%Y-%m-%d{"}"} a data da foto, {"{"}w{"}"} e {"{"}h{"}"} o novo tamanho, {"{"}ext{"}"} a extensão do formato.
settings-convert-conflicts = Quando um nome já existe
settings-convert-conflicts-help = Arquivos existentes nunca são substituídos.
settings-convert-conflicts-suffix = Adicionar um número
settings-convert-conflicts-skip = Pular a imagem

## The dialog
convert-title = Converter
convert-images = Imagens
convert-images-current = Esta imagem
convert-images-selection = Seleção ({ $count })
convert-images-folder = Pasta ({ $count })
convert-images-picked = Escolhidas ({ $count })
convert-quality = Qualidade
convert-chroma = Detalhe de cor
convert-chroma-half = 4:2:0 (menor)
convert-chroma-full = 4:4:4 (mais nítido)
convert-effort = Esforço
convert-on = Ligado
convert-off = Desligado
convert-pixels = { $value } px
convert-effort-fastest = { $value } (mais rápido)
convert-folder-subfolder = Subpasta “{ $name }”
convert-folder-same = Mesma pasta da imagem
convert-choose-folder = Escolher pasta…
convert-name = Nome
convert-name-preview = { $source } → { $output }
convert-name-error-unclosed = Uma {"{"} não foi fechada.
convert-name-error-stray = Uma {"}"} não tem {"{"}. Escreva {"}}"} para uma chave.
convert-name-error-unknown = { $token } não é uma parte do nome. Use name, n, date, w, h ou ext.
convert-name-error-format = { $token } tem um formato inválido.
convert-name-error-empty = Digite um padrão de nome.
convert-start = Converter
convert-stop = Parar
convert-hide = Ocultar
convert-close = Fechar
convert-progress = Convertendo { $done } de { $total }
convert-progress-eta = Convertendo { $done } de { $total } · faltam cerca de { $time }
convert-eta-seconds = { $count ->
    [one] { $count } segundo
   *[other] { $count } segundos
}
convert-eta-minutes = { $count ->
    [one] { $count } minuto
   *[other] { $count } minutos
}
convert-done = { $count ->
    [one] { $count } imagem convertida
   *[other] { $count } imagens convertidas
}
convert-done-skipped = { $count } puladas: o nome já existia
convert-done-failed = { $count } com falha
convert-stopped = Parado após { $count }
convert-failure = { $file }: { $reason }
convert-failure-open = não pode ser aberto ({ $error })
convert-failure-decode = não pode ser lido ({ $error })
convert-failure-encode = não pode ser gravado neste formato ({ $error })
convert-failure-write = não pode ser salvo ({ $error })
convert-failure-same = o novo arquivo substituiria o original
convert-report-more = e mais { $count }
convert-running = Já há uma conversão em andamento.

## Convert encoders (machine-drafted, needs native review)
settings-convert-format-avif = AVIF
settings-convert-webp-lossless = WebP sem perdas
settings-convert-webp-lossless-help = Mantém cada pixel exatamente. Desativado, os arquivos WebP ficam bem menores na qualidade WebP.
settings-convert-webp-quality = Qualidade WebP
settings-convert-webp-quality-help = Valores mais altos mantêm mais detalhes em arquivos maiores. Usado quando o WebP não é sem perdas.
settings-convert-webp-effort = Esforço do WebP
settings-convert-webp-effort-help = Valores mais altos geram arquivos menores, mais devagar. Usado quando o WebP não é sem perdas.
settings-convert-jxl-lossless = JPEG XL sem perdas
settings-convert-jxl-lossless-help = Mantém cada pixel exatamente. Desativado, os arquivos JPEG XL ficam bem menores na qualidade JPEG XL.
settings-convert-jxl-quality = Qualidade JPEG XL
settings-convert-jxl-quality-help = Valores mais altos mantêm mais detalhes em arquivos maiores. Com 90 é difícil distinguir do original. Usado quando o JPEG XL não é sem perdas.
settings-convert-avif-quality = Qualidade AVIF
settings-convert-avif-quality-help = Valores mais altos mantêm mais detalhes em arquivos maiores.
settings-convert-avif-effort = Esforço do AVIF
settings-convert-avif-effort-help = Valores mais altos geram arquivos menores, mais devagar. 1 é o mais rápido.
convert-lossless = Sem perdas
