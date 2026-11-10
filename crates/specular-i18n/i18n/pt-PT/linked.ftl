# Linked views: several DICOM series side by side, scrolling together by position, with
# reference lines, a 3D cursor, window and zoom sync, and manual alignment of priors.

# machine-drafted, needs native review
palette-compare-series = Comparar séries (vistas ligadas)
# machine-drafted, needs native review
palette-compare-layout = Disposição de vistas seguinte
# machine-drafted, needs native review
palette-next-viewport = Vista seguinte
# machine-drafted, needs native review
palette-prev-viewport = Vista anterior
# machine-drafted, needs native review
palette-reference-lines = Mostrar ou ocultar as linhas de referência
# machine-drafted, needs native review
palette-sync-window = Sinc. de janela seguinte
# machine-drafted, needs native review
palette-sync-zoom = Zoom e deslocamento em conjunto por tamanho
# machine-drafted, needs native review
palette-link-series = Ligação de série seguinte
# machine-drafted, needs native review
palette-align-series = Alinhar aqui as outras séries
# machine-drafted, needs native review
palette-compare-series-help = Mostrar várias séries lado a lado, a deslocar em conjunto.
# machine-drafted, needs native review
palette-compare-layout-help = Mostrar mais ou menos vistas.
# machine-drafted, needs native review
palette-next-viewport-help = Tornar ativa a vista seguinte.
# machine-drafted, needs native review
palette-prev-viewport-help = Tornar ativa a vista anterior.
# machine-drafted, needs native review
palette-reference-lines-help = Desenhar onde o corte ativo cruza as outras vistas.
# machine-drafted, needs native review
palette-sync-window-help = Escolher que vistas partilham uma alteração de janela/nível: desligada, mesma modalidade ou todas.
# machine-drafted, needs native review
palette-sync-zoom-help = Mostrar todas as vistas com o mesmo tamanho e deslocá-las em conjunto.
# machine-drafted, needs native review
palette-link-series-help = Escolher como esta série acompanha as outras.
# machine-drafted, needs native review
palette-align-series-help = Alinhar uma série anterior com esta nos cortes apresentados.
# machine-drafted, needs native review
disabled-not-comparing = Compare séries primeiro

# The bar above the viewports. Toggles show their state by colour; the others name it.
# machine-drafted, needs native review
linked-layout = Disposição: { $cols } × { $rows }
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
linked-link = Ligação: { $mode ->
    [frame] referencial
    [position] posição do doente
   *[off] desligada
}
# machine-drafted, needs native review
linked-align = Alinhar aqui

# One viewport. $active is "yes" for the active one.
# machine-drafted, needs native review
linked-viewport = { $active ->
    [yes] Vista { $n } de { $count }, ativa
   *[no] Vista { $n } de { $count }
}
# machine-drafted, needs native review
linked-empty = Arraste uma série para aqui
# machine-drafted, needs native review
linked-series = { $number } · { $modality } · { $description }
# machine-drafted, needs native review
linked-slice = Corte { $n } de { $count } · { $position } mm
# machine-drafted, needs native review
linked-in-gap = Num intervalo: corte mais próximo a { $distance } mm
# machine-drafted, needs native review
linked-outside = Fora desta série ({ $distance } mm)
# machine-drafted, needs native review
linked-not-linked = Não ligada
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
    [one] Só existe uma série aqui: arraste outra do navegador de estudos
   *[other] A comparar { $count } séries
}
# machine-drafted, needs native review
linked-no-geometry = Esta série não tem posições de corte: não pode ser ligada
# machine-drafted, needs native review
linked-active = Vista { $n }: { $series }
# machine-drafted, needs native review
linked-aligned-count = { $count ->
    [0] Nada a alinhar: as outras séries partilham este referencial
    [one] 1 série alinhada com a ativa, nesta sessão
   *[other] { $count } séries alinhadas com a ativa, nesta sessão
}
