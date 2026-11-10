# Specular — Drawings (DXF): layers, paper, lineweights, model space and layouts. Português (Portugal).

keys-mode-drawing = Desenhos
cheatsheet-drawing = Desenhos
palette-drawing-layers = Mostrar ou ocultar o painel de camadas
palette-drawing-all-layers = Mostrar todas as camadas ou as camadas tal como guardadas
palette-drawing-paper = Trocar papel escuro e claro
palette-drawing-lineweights = Mostrar ou ocultar as espessuras de linha
disabled-not-drawing = Apenas para desenhos (DXF)
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
drawing-layers-saved-toast = Camadas tal como guardadas no desenho
drawing-paper-dark-toast = Papel escuro
drawing-paper-light-toast = Papel claro
drawing-lineweights-on = Espessuras de linha visíveis
drawing-lineweights-off = Espessuras de linha desativadas: linhas finas

# machine-drafted, needs native review
# Markup on drawings and images (Alt+M): kept in a file next to the drawing, never in it.
palette-markup = Anotar
palette-markup-help = Nuvens, texto, setas, carimbos, medições e marcadores de fotografia, guardados ao lado do ficheiro.
palette-compare-sheets = Comparar folhas
palette-compare-sheets-help = Sobrepor outra revisão desta folha, alinhada, com as alterações assinaladas.
markup-title = Anotações
markup-list = Anotações nesta página
markup-count = { $count ->
    [0] Nenhuma anotação nesta página
    [one] 1 anotação nesta página
   *[other] { $count } anotações nesta página
}
markup-empty = Escolha uma ferramenta e clique na folha.
markup-item = { $n }. { $kind }
markup-tool-select = Selecionar
markup-tool-cloud = Nuvem
markup-tool-text = Texto
markup-tool-arrow = Seta
markup-tool-stamp = Carimbo
markup-tool-length = Comprimento
markup-tool-area = Área
markup-tool-pin = Marcador de fotografia
markup-hint-select = Clique num elemento para o selecionar. Arraste para o mover. Enter edita o texto.
markup-hint-cloud = Arraste um retângulo, ou clique nos cantos e prima Enter.
markup-hint-text = Clique onde fica o texto.
markup-hint-arrow = Arraste da etiqueta até ao ponto, ou clique em ambos.
markup-hint-stamp = Clique onde fica o carimbo.
markup-hint-length = Arraste, ou clique nas duas extremidades.
markup-hint-area = Clique nos cantos e prima Enter.
markup-hint-pin = Clique num ponto e escolha uma fotografia.
markup-kind-cloud = Nuvem
markup-kind-text = Texto
markup-kind-arrow = Seta
markup-kind-stamp = Carimbo
markup-kind-length = Comprimento
markup-kind-area = Área
markup-kind-pin = Fotografia
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
markup-stamp-rejected = Rejeitado
markup-stamp-reviewed = Revisto
markup-stamp-custom = Texto próprio
markup-undo = Anular
markup-redo = Refazer
markup-delete = Eliminar
markup-open-photo = Abrir fotografia
markup-from-measurements = Adicionar medições
markup-from-measurements-done = { $count ->
    [one] 1 medição adicionada como anotação
   *[other] { $count } medições adicionadas como anotações
}
markup-export-pdf = Exportar PDF…
markup-export-png = Exportar PNG…
markup-done = Concluído
markup-ask-text = Texto (Enter para colocar, Esc para cancelar)
markup-ask-label = Etiqueta (opcional; Enter para colocar)
markup-ask-stamp = Texto do carimbo (Enter para colocar)
markup-ask-edit = Novo texto (Enter para guardar)
markup-added = Adicionado: { $kind }
markup-moved = Movido
markup-pin-hint = Clique de novo no marcador ou prima Enter para abrir a fotografia.
markup-deleted = Eliminado
markup-undone = Anulado
markup-redone = Refeito
markup-delete-ask = Eliminar { $name }?
markup-delete-detail = O elemento é removido do ficheiro de anotações. O desenho em si não é alterado.
markup-delete-ok = Eliminar
markup-photo-missing = { $name } já não existe.
markup-load-failed = Anotações não lidas: { $reason }
markup-save-failed = Anotações não guardadas: { $reason }
markup-sidecar-too-large = o ficheiro de anotações é demasiado grande
markup-pdf-unreadable = não foi possível ler o PDF
markup-exported = { $name } guardado
markup-export-failed = Não exportado: { $reason }
markup-export-same-file = escolha outro nome; o original nunca é substituído
markup-export-encrypted = o PDF está encriptado. Exporte antes em PNG.
markup-export-damaged = a estrutura do PDF está danificada. Exporte antes em PNG.
sheets-title = Comparar folhas
sheets-other = Outra folha…
sheets-other-named = Outra: { $name }
sheets-page = Página { $page } de { $count }
sheets-auto = Alinhar
sheets-pick = Escolher 2 pontos
sheets-overlay = Sobreposição
sheets-difference = Diferença
sheets-blend = Desvanecer
sheets-blink = Intermitente
sheets-slider = Outra folha { PERCENT($percent) }
sheets-clouds = { $count ->
    [0] Nenhuma alteração
    [one] Assinalar 1 alteração com nuvem
   *[other] Assinalar { $count } alterações com nuvens
}
sheets-done = Concluído
sheets-choose = Escolha a outra folha.
sheets-reading = A ler a folha…
sheets-aligning = A alinhar…
sheets-aligned = Alinhada: escala { $scale }, rotação { $angle }°, ajuste até { $error } px. { $changes ->
    [0] Nenhuma alteração encontrada.
    [one] 1 alteração.
   *[other] { $changes } alterações.
}
sheets-align-failed = Não foi possível alinhar automaticamente. Escolha 2 pontos.
sheets-pick-this = Ponto { $n }: clique nele nesta folha.
sheets-pick-other = Ponto { $n }: clique no mesmo ponto na outra folha.
sheets-pick-same = Os dois pontos são iguais. Escolha 2 pontos de novo.
sheets-read-failed = Folha não lida: { $reason }
sheets-failed = Esse passo falhou. Tente de novo ou escolha outra folha.
sheets-legend-this = Só esta folha
sheets-legend-other = Só a outra folha
sheets-legend-both = Ambas
sheets-cloud-text = Alterado
sheets-clouds-added = { $count ->
    [0] Nenhuma nuvem adicionada
    [one] 1 nuvem adicionada às anotações
   *[other] { $count } nuvens adicionadas às anotações
}
sheets-opened = Comparar folhas
sheets-closed = Comparação fechada
markup-ask-new-line = Shift+Enter: nova linha

## Settings › Drawings

# machine-drafted, needs native review
settings-section-drawings = Desenhos
settings-drawings-shx-folders = Pastas de tipos de letra SHX
settings-drawings-shx-folders-help = Pastas com tipos de letra SHX para o texto de DXF e DWG, separadas por ;. A pasta do próprio desenho é pesquisada primeiro.
# A folder in the SHX font folders setting that does not exist or is not a folder.
settings-error-shx-folder = Não é uma pasta: { $folder }
