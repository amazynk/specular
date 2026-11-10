# Linked views: several DICOM series side by side, scrolling together by position, with
# reference lines, a 3D cursor, window and zoom sync, and manual alignment of priors.

# machine-drafted, needs native review
palette-compare-series = Comparar séries (visualizações vinculadas)
# machine-drafted, needs native review
palette-compare-layout = Próximo layout de visualizações
# machine-drafted, needs native review
palette-next-viewport = Próxima visualização
# machine-drafted, needs native review
palette-prev-viewport = Visualização anterior
# machine-drafted, needs native review
palette-reference-lines = Mostrar ou ocultar as linhas de referência
# machine-drafted, needs native review
palette-sync-window = Próxima sinc. de janela
# machine-drafted, needs native review
palette-sync-zoom = Zoom e deslocamento juntos por tamanho
# machine-drafted, needs native review
palette-link-series = Próximo vínculo de série
# machine-drafted, needs native review
palette-align-series = Alinhar aqui as outras séries
# machine-drafted, needs native review
palette-compare-series-help = Mostrar várias séries lado a lado, rolando juntas.
# machine-drafted, needs native review
palette-compare-layout-help = Mostrar mais ou menos visualizações.
# machine-drafted, needs native review
palette-next-viewport-help = Tornar ativa a próxima visualização.
# machine-drafted, needs native review
palette-prev-viewport-help = Tornar ativa a visualização anterior.
# machine-drafted, needs native review
palette-reference-lines-help = Desenhar onde o corte ativo cruza as outras visualizações.
# machine-drafted, needs native review
palette-sync-window-help = Escolher quais visualizações compartilham uma mudança de janela/nível: desligada, mesma modalidade ou todas.
# machine-drafted, needs native review
palette-sync-zoom-help = Mostrar todas as visualizações no mesmo tamanho e deslocá-las juntas.
# machine-drafted, needs native review
palette-link-series-help = Escolher como esta série acompanha as outras.
# machine-drafted, needs native review
palette-align-series-help = Alinhar uma série anterior a esta nos cortes exibidos.
# machine-drafted, needs native review
disabled-not-comparing = Compare séries primeiro

# The bar above the viewports. Toggles show their state by colour; the others name it.
# machine-drafted, needs native review
linked-layout = Layout: { $cols } × { $rows }
# machine-drafted, needs native review
linked-lines = Linhas de referência
# machine-drafted, needs native review
linked-zoom = Zoom sincronizado
# machine-drafted, needs native review
linked-window-sync = Janela sincronizada: { $mode ->
    [off] desligada
    [modality] mesma modalidade
   *[all] todas
}
# machine-drafted, needs native review
linked-link = Vínculo: { $mode ->
    [frame] referencial
    [position] posição do paciente
   *[off] desligado
}
# machine-drafted, needs native review
linked-align = Alinhar aqui

# One viewport. $active is "yes" for the active one.
# machine-drafted, needs native review
linked-viewport = { $active ->
    [yes] Visualização { $n } de { $count }, ativa
   *[no] Visualização { $n } de { $count }
}
# machine-drafted, needs native review
linked-empty = Arraste uma série para cá
# machine-drafted, needs native review
linked-series = { $number } · { $modality } · { $description }
# machine-drafted, needs native review
linked-slice = Corte { $n } de { $count } · { $position } mm
# machine-drafted, needs native review
linked-in-gap = Em um intervalo: corte mais próximo a { $distance } mm
# machine-drafted, needs native review
linked-outside = Fora desta série ({ $distance } mm)
# machine-drafted, needs native review
linked-not-linked = Não vinculada
# machine-drafted, needs native review
linked-aligned = Alinhada manualmente ({ $offset } mm)
# machine-drafted, needs native review
linked-cursor = Cursor 3D em { $x }, { $y }, { $z } mm
# machine-drafted, needs native review
linked-chip = { $label }, { $count ->
        [one] { $count } imagem
       *[other] { $count } imagens
    }

# Toasts and announcements.
# machine-drafted, needs native review
linked-opened = { $count ->
    [one] Só há uma série aqui: arraste outra do navegador de estudos
   *[other] Comparando { $count } séries
}
# machine-drafted, needs native review
linked-no-geometry = Esta série não tem posições de corte: não pode ser vinculada
# machine-drafted, needs native review
linked-active = Visualização { $n }: { $series }
# machine-drafted, needs native review
linked-aligned-count = { $count ->
    [0] Nada a alinhar: as outras séries compartilham este referencial
    [one] 1 série alinhada com a ativa, nesta sessão
   *[other] { $count } séries alinhadas com a ativa, nesta sessão
}
