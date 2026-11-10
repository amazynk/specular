# Specular — Export slideshow as video (DESIGN.md §6.5). Portuguese (Portugal). Machine-drafted, needs native review.

palette-export-slideshow = Exportar apresentação de diapositivos como vídeo…

video-export-title = Exportar apresentação de diapositivos como vídeo
video-export-resolution = Resolução
# $width and $height in pixels.
video-export-resolution-window = Janela ({ $width } × { $height })
video-export-fps = Fotogramas por segundo
# $fps frames per second, a number.
video-export-fps-value = { $fps } fps
video-export-quality = Qualidade
video-export-quality-high = Alta
video-export-quality-standard = Normal
video-export-quality-compact = Compacta
video-export-images = { $count ->
    [one] { $count } imagem, com a ordem, a transição e as legendas da apresentação
   *[other] { $count } imagens, com a ordem, a transição e as legendas da apresentação
}
# $length a duration ("2:40"), $size a file size ("38 MB").
video-export-summary = Cerca de { $length } de duração, cerca de { $size }
video-export-export = Exportar…
video-export-replace = Substituir
# $file a file name.
video-export-replace-question = { $file } já existe. Substituir?
# $percent a formatted percentage ("42%").
video-export-progress = A exportar… { $percent }
video-export-cancelling = A cancelar…
# $file a file name, $size a file size ("38 MB").
video-export-saved = { $file } guardado ({ $size })
video-export-skipped = { $count ->
    [one] { $count } imagem não pôde ser lida e foi deixada de fora.
   *[other] { $count } imagens não puderam ser lidas e foram deixadas de fora.
}
video-export-failed = Não foi possível exportar o vídeo: { $reason }
video-export-cancelled = Exportação cancelada
video-export-no-images = Não foi possível ler nenhuma das imagens.
video-export-show-in-folder = Mostrar na pasta

## Toolbar item and compatibility (machine-drafted, needs native review)
settings-viewing-toolbar-items-export-slideshow = Exportar apresentação como vídeo
video-export-compatibility = Compatibilidade
video-export-compatibility-current = Leitores atuais
video-export-compatibility-all = Todos os leitores
