# Specular — Drawings (DXF): layers, paper, lineweights, model space and layouts. Português (Brasil).

keys-mode-drawing = Desenhos
cheatsheet-drawing = Desenhos
palette-drawing-layers = Mostrar ou ocultar o painel de camadas
palette-drawing-all-layers = Mostrar todas as camadas ou as camadas como salvas
palette-drawing-paper = Alternar papel escuro e claro
palette-drawing-lineweights = Mostrar ou ocultar as espessuras de linha
disabled-not-drawing = Somente para desenhos (DXF)
# A drawing's page: model space (layouts are shown by their own name).
page-label-model = Modelo
# machine-drafted, needs native review
cad-pen-name = Caneta { $n }
cad-page-name = Página { $n }
cad-layer-name = Camada { $n }
cad-map-name = Mapa
# machine-drafted, needs native review
# A board's views (its Gerber files together): every layer stacked, and its top and bottom
# in board colours.
cad-board-layers-name = Camadas
cad-board-top-name = Superior
cad-board-bottom-name = Inferior
# machine-drafted, needs native review
# A plot's layers: its frame, a trace without a name (numbered from 1), and the contour
# lines of a 2-D spectrum above and below zero.
cad-chart-frame-name = Moldura
cad-trace-name = Traço { $n }
cad-contours-positive-name = Contornos positivos
cad-contours-negative-name = Contornos negativos
# machine-drafted, needs native review
# The page of a survey file (LandXML) showing its surfaces and alignments in 3D.
cad-model-3d-name = Modelo 3D
# The layers panel's title (and its name for screen readers).
drawing-layers-title = Camadas · { $shown } de { $total } visíveis
# Toasts.
drawing-layers-all-toast = Todas as camadas visíveis
drawing-layers-saved-toast = Camadas como salvas no desenho
drawing-paper-dark-toast = Papel escuro
drawing-paper-light-toast = Papel claro
drawing-lineweights-on = Espessuras de linha visíveis
drawing-lineweights-off = Espessuras de linha desativadas: linhas finas

# machine-drafted, needs native review
# Markup on drawings and images (Alt+M): kept in a file next to the drawing, never in it.
palette-markup = Marcar
palette-markup-help = Nuvens, texto, setas, carimbos, medições e marcadores de foto, salvos ao lado do arquivo.
palette-compare-sheets = Comparar pranchas
palette-compare-sheets-help = Sobrepor outra revisão desta prancha, alinhada, com as alterações destacadas.
markup-title = Marcações
markup-list = Marcações nesta página
markup-count = { $count ->
    [0] Nenhuma marcação nesta página
    [one] { $count } marcação nesta página
   *[other] { $count } marcações nesta página
}
markup-empty = Escolha uma ferramenta e clique na prancha.
markup-item = { $n }. { $kind }
markup-tool-select = Selecionar
markup-tool-cloud = Nuvem
markup-tool-text = Texto
markup-tool-arrow = Seta
markup-tool-stamp = Carimbo
markup-tool-length = Comprimento
markup-tool-area = Área
markup-tool-pin = Marcador de foto
markup-hint-select = Clique em um item para selecioná-lo. Arraste para movê-lo. Enter edita o texto.
markup-hint-cloud = Arraste um retângulo, ou clique nos cantos e pressione Enter.
markup-hint-text = Clique onde fica o texto.
markup-hint-arrow = Arraste do rótulo até o ponto, ou clique nos dois.
markup-hint-stamp = Clique onde fica o carimbo.
markup-hint-length = Arraste, ou clique nas duas pontas.
markup-hint-area = Clique nos cantos e pressione Enter.
markup-hint-pin = Clique em um ponto e escolha uma foto.
markup-kind-cloud = Nuvem
markup-kind-text = Texto
markup-kind-arrow = Seta
markup-kind-stamp = Carimbo
markup-kind-length = Comprimento
markup-kind-area = Área
markup-kind-pin = Foto
markup-colour = Cor: { $colour }
markup-colour-red = Vermelho
markup-colour-blue = Azul
markup-colour-green = Verde
markup-colour-orange = Laranja
markup-colour-magenta = Magenta
markup-colour-black = Preto
markup-width = Linha: { $width }
markup-width-thin = Fina
markup-width-medium = Média
markup-width-thick = Grossa
markup-stamp = Carimbo: { $words }
markup-stamp-approved = Aprovado
markup-stamp-rejected = Reprovado
markup-stamp-reviewed = Revisado
markup-stamp-custom = Texto próprio
markup-undo = Desfazer
markup-redo = Refazer
markup-delete = Excluir
markup-open-photo = Abrir foto
markup-from-measurements = Adicionar medições
markup-from-measurements-done = { $count ->
    [one] { $count } medição adicionada como marcação
   *[other] { $count } medições adicionadas como marcações
}
markup-export-pdf = Exportar PDF…
markup-export-png = Exportar PNG…
markup-done = Concluído
markup-ask-text = Texto (Enter para colocar, Esc para cancelar)
markup-ask-label = Rótulo (opcional; Enter para colocar)
markup-ask-stamp = Texto do carimbo (Enter para colocar)
markup-ask-edit = Novo texto (Enter para salvar)
markup-added = Adicionado: { $kind }
markup-moved = Movido
markup-pin-hint = Clique de novo no marcador ou pressione Enter para abrir a foto.
markup-deleted = Excluído
markup-undone = Desfeito
markup-redone = Refeito
markup-delete-ask = Excluir { $name }?
markup-delete-detail = O item é removido do arquivo de marcações. O desenho em si não é alterado.
markup-delete-ok = Excluir
markup-photo-missing = { $name } não está mais lá.
markup-load-failed = Marcações não lidas: { $reason }
markup-save-failed = Marcações não salvas: { $reason }
markup-sidecar-too-large = o arquivo de marcações é grande demais
markup-pdf-unreadable = não foi possível ler o PDF
markup-exported = { $name } salvo
markup-export-failed = Não exportado: { $reason }
markup-export-same-file = escolha outro nome; o original nunca é sobrescrito
markup-export-encrypted = o PDF está criptografado. Exporte PNG em vez disso.
markup-export-damaged = a estrutura do PDF está danificada. Exporte PNG em vez disso.
sheets-title = Comparar pranchas
sheets-other = Outra prancha…
sheets-other-named = Outra: { $name }
sheets-page = Página { $page } de { $count }
sheets-auto = Alinhar
sheets-pick = Escolher 2 pontos
sheets-overlay = Sobreposição
sheets-difference = Diferença
sheets-blend = Esmaecer
sheets-blink = Piscar
sheets-slider = Outra prancha { PERCENT($percent) }
sheets-clouds = { $count ->
    [0] Nenhuma alteração
    [one] Marcar { $count } alteração com nuvem
   *[other] Marcar { $count } alterações com nuvens
}
sheets-done = Concluído
sheets-choose = Escolha a outra prancha.
sheets-reading = Lendo a prancha…
sheets-aligning = Alinhando…
sheets-aligned = Alinhada: escala { $scale }, giro { $angle }°, ajuste dentro de { $error } px. { $changes ->
    [0] Nenhuma alteração encontrada.
    [one] { $changes } alteração.
   *[other] { $changes } alterações.
}
sheets-align-failed = Não foi possível alinhar automaticamente. Escolha 2 pontos.
sheets-pick-this = Ponto { $n }: clique nele nesta prancha.
sheets-pick-other = Ponto { $n }: clique no mesmo ponto na outra prancha.
sheets-pick-same = Os dois pontos são iguais. Escolha 2 pontos de novo.
sheets-read-failed = Prancha não lida: { $reason }
sheets-failed = Essa etapa falhou. Tente de novo ou escolha outra prancha.
sheets-legend-this = Só esta prancha
sheets-legend-other = Só a outra prancha
sheets-legend-both = Ambas
sheets-cloud-text = Alterado
sheets-clouds-added = { $count ->
    [0] Nenhuma nuvem adicionada
    [one] { $count } nuvem adicionada às marcações
   *[other] { $count } nuvens adicionadas às marcações
}
sheets-opened = Comparar pranchas
sheets-closed = Comparação fechada
markup-ask-new-line = Shift+Enter: nova linha

## Settings › Drawings

# machine-drafted, needs native review
settings-section-drawings = Desenhos
settings-drawings-shx-folders = Pastas de fontes SHX
settings-drawings-shx-folders-help = Pastas com fontes SHX para o texto de DXF e DWG, separadas por ;. A pasta do próprio desenho é pesquisada primeiro.
# A folder in the SHX font folders setting that does not exist or is not a folder.
settings-error-shx-folder = Não é uma pasta: { $folder }
