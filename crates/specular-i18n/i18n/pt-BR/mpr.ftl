# Multiplanar reformatting (MPR).

palette-mpr = Reconstrução multiplanar (MPR)
palette-mpr-slab = Próxima espessura MPR (fina, MIP, MinIP, média)
disabled-no-volume = Isto não é um volume: abra uma série, um arquivo multiquadro, um volume 3D ou uma pilha z
disabled-mpr-closed = Abra primeiro as vistas multiplanares
mpr-axial = Axial
mpr-coronal = Coronal
mpr-sagittal = Sagital
mpr-oblique = Oblíquo
mpr-xy = XY
mpr-xz = XZ
mpr-yz = YZ
mpr-3d = Vista 3D
mpr-thin = Fina
mpr-mip = MIP
mpr-minip = MinIP
mpr-average = Média
mpr-slab = Espessura: { $mode }, { $thickness } mm
# machine-drafted, needs native review
mpr-slab-angstrom = Espessura: { $mode }, { $thickness } Å
mpr-slab-unit = Espessura: { $mode }, { $thickness } { $unit }
mpr-slab-voxels = Espessura: { $mode }, { $thickness } voxels
mpr-thicker = Espessura maior
mpr-thinner = Espessura menor
mpr-reset = Redefinir vistas
mpr-close = Fechar MPR
mpr-maximise = Maximizar vista
mpr-export-planes = Exportar planos
# machine-drafted, needs native review
# Streamlines (TrackVis .trk, MRtrix .tck) found beside a brain volume, drawn over each
# view within a slab about its plane; Alt+S steps the slab, then hides them. $files: the
# files' names; $thickness: the slab in millimetres.
mpr-tracts = Linhas de corrente de { $files }: espessura { $thickness } mm
mpr-tracts-off = Linhas de corrente de { $files }: ocultas
mpr-tracts-none = Nenhuma linha de corrente TrackVis ou MRtrix ao lado deste volume
mpr-position = { $view } { $position } mm
# machine-drafted, needs native review
mpr-position-angstrom = { $view } { $position } Å
mpr-position-unit = { $view } { $position } { $unit }
mpr-position-voxels = { $view } { $position }
mpr-length-unit = { $value } { $unit }
mpr-length-voxels = { $value } voxels
mpr-axis-label = { $axis }: { $label }
mpr-axis-position = { $axis }: { $position } de { $count }
mpr-axis-channel = Canal
mpr-axis-unnamed = Eixo { $axis }
mpr-one-channel = A MPR mostra um canal por vez
# machine-drafted, needs native review
# $count: channels drawn together in their colours.
mpr-composite = { $count } canais em cores, cada um no seu intervalo de exibição
# $reason: error-reason-needs-memory ("it needs 6 GB of free RAM, but only 4 GB is free right now. ...").
mpr-composite-memory = Um canal mostrado: { $reason }
mpr-isotropic = O arquivo não informa o tamanho do voxel: voxels mostrados como cubos
mpr-isotropic-z = O arquivo não informa o espaçamento entre cortes: cortes mostrados a um pixel de distância
# A seismic volume's inline and crossline sections (the time slice reads "Time … ms").
mpr-seismic-inline = Inline { $inline }
mpr-seismic-crossline = Crossline { $crossline }
mpr-value = { $value } { $unit }
mpr-value-plain = { $value }
mpr-loading = Carregando { $loaded } de { $count ->
        [one] { $count } corte
       *[other] { $count } cortes
    }
mpr-loaded = { $count ->
        [one] { $count } corte
       *[other] { $count } cortes
    }, { $width } × { $height }
mpr-downsampled = Exibido a 1/{ $factor } da resolução (limites de memória ou gráficos)
mpr-converted = Valores exibidos como floats de 32 bits
mpr-opening = Abrindo o volume…
mpr-gpu-memory = { $size } na placa de vídeo
mpr-refused = Não é possível montar um volume: { $reason }
mpr-failed = O volume não carregou: { $reason }
mpr-no-gpu = A placa de vídeo não comporta este volume: { $reason }
mpr-planes-saved = { $count ->
        [one] { $count } plano salvo
       *[other] { $count } planos salvos
    } em { $folder }
mpr-planes-failed = Não foi possível salvar os planos: { $reason }
mpr-view-a11y = Vista { $view }, { $position } mm
mpr-crosshair-a11y = Mira em { $x }, { $y }, { $z } mm
# machine-drafted, needs native review
mpr-crosshair-a11y-angstrom = Mira em { $x }, { $y }, { $z } Å
mpr-crosshair-a11y-unit = Mira em { $x }, { $y }, { $z } { $unit }
mpr-crosshair-a11y-voxels = Mira no voxel { $x }, { $y }, { $z }
mpr-controls = Controles de MPR

# 3D view palette entries.
palette-mpr-preset = Próxima predefinição 3D (TC osso, partes moles, pulmão, angio; RM)
palette-mpr-render = Próxima renderização 3D (sombreada, Cinematic, MIP, MinIP)
palette-mpr-clip = Próximo recorte 3D (nenhum, caixa das vistas 2D, canto removido)
palette-mpr-export-stl = Exportar a superfície 3D como STL…
# machine-drafted, needs native review
palette-mpr-tract-slab = Próxima espessura das linhas de corrente na MPR
palette-mpr-maximise = Maximizar a vista MPR ou mostrar todas as vistas
palette-mpr-export-planes = Exportar os planos MPR como imagens…

# The 3D view: transfer function presets, renderings, clipping; STL surface export.
vr-preset = { $preset ->
    [bone] TC osso
    [soft] TC partes moles
    [lung] TC pulmão
    [angio] Angio-TC
   *[mr] RM
}
vr-mode = { $mode ->
    [cinematic] Cinematic
    [mip] MIP
    [minip] MinIP
   *[shaded] Sombreada
}
vr-clip = { $clip ->
    [box] Recorte: caixa das vistas
    [cut] Recorte: canto removido
   *[off] Sem recorte
}
vr-smooth = Suavizar a superfície
vr-export = Exportar STL
vr-threshold = Superfície em { $value }
vr-stl-exporting = Construindo a superfície…
vr-stl-saved = { $count ->
    [0] Nada neste limiar: nenhuma superfície salva
   *[other] { $count } triângulos salvos em { $name }
}
vr-stl-failed = Não foi possível salvar a superfície: { $reason }
vr-view-a11y = { $preset }, { $mode }, { $clip }, superfície em { $threshold }. Shift+setas para orbitar, mais e menos para zoom.

## Volume bruto: como um arquivo .raw / .vol sem cabeçalho está organizado

# machine-drafted, needs native review
raw-volume-title = Abrir volume bruto
raw-volume-file = { $file } não tem cabeçalho. Informe como as amostras estão organizadas.
raw-volume-guess-label = Organização sugerida
raw-volume-guess = { $index } de { $count }
raw-volume-custom = Personalizada
raw-volume-width = Largura (voxels)
raw-volume-height = Altura (voxels)
raw-volume-slices = Cortes
raw-volume-type = Tipo de amostra
raw-volume-type-unsigned = Sem sinal de { $bits } bits
raw-volume-type-signed = Com sinal de { $bits } bits
raw-volume-type-float = Ponto flutuante de { $bits } bits
raw-volume-order = Ordem dos bytes
raw-volume-little = Little-endian
raw-volume-big = Big-endian
raw-volume-offset = Deslocamento do cabeçalho
raw-volume-bytes = { $count ->
    [one] { $count } byte
   *[other] { $count } bytes
}
raw-volume-pixel = Tamanho do pixel
raw-volume-spacing = Espaçamento entre cortes
raw-volume-mm = { $value } mm
raw-volume-exact = Preenche o arquivo exatamente ({ $size })
raw-volume-fits = Usa { $size } de { $file }; sobram { $left }
raw-volume-too-big = Precisa de { $size }, mas o arquivo tem { $file }
raw-volume-cancel = Cancelar
raw-volume-open = Abrir
# machine-drafted, needs native review
# A dense connectivity file (CIFTI .dconn.nii) over voxels: the volume shown is one seed's
# row; letting go of the crosshair on another seed's voxel shows that one. $seed: the
# structure as the file names it and the voxel, such as "THALAMUS_LEFT (4, 6, 3)".
mpr-dconn-seed = Conectividade de { $seed }; mova a cruz para outra semente
# machine-drafted, needs native review
mpr-measure-hint-edit = Arraste uma ponta para movê-la; sobre a outra ponta para remover
# machine-drafted, needs native review
mpr-measure-removed = Medida removida
mpr-measure-lengths-only = A MPR mede apenas comprimentos
mpr-more = Mais…

## Curved MPR: a reformat along a curve clicked on a plane.
## machine-drafted, needs native review

palette-mpr-curve = MPR curva ao longo de uma curva clicada
palette-mpr-curve-layout = MPR curva: retificada ou esticada
palette-mpr-curve-save = Salvar a curva…
palette-mpr-curve-load = Carregar uma curva…
palette-mpr-export-segments = Exportar cada segmento como STL…
palette-mpr-dental = Layout odontológico (panorâmica e cortes transversais)
palette-mpr-nerve = Traçar o canal mandibular
cpr-kind = { $kind ->
    [stretched] Esticada
   *[straightened] Retificada
}
cpr-button = Curva
cpr-controls = Controles da MPR curva
cpr-at = Em { $position }
cpr-section = Corte transversal
cpr-hint-start = Clique em pontos ao longo da estrutura na vista { $view }
cpr-hint-more = Clique em mais um ponto
cpr-add-points = Adicionar pontos
cpr-undo-point = Desfazer ponto
# machine-drafted, needs native review
cpr-mark = Marcar aqui
cpr-mark-set = Marca { $n } definida.
cpr-mark-removed = Marca removida.
cpr-mark-here = Marca { $n }
cpr-diameter = Diâmetro { $value }
cpr-diameters = Diâmetro de { $min } a { $max }
cpr-mark-span = Marcas { $from } a { $to }: { $length } ao longo da linha central
cpr-mark-total = Todas as marcas: { $length }
cpr-clear = Limpar curva
cpr-save = Salvar curva
cpr-load = Carregar curva
cpr-done = Concluído
cpr-a11y = { $name } ao longo de { $length }, cursor em { $position }. A roda move o cursor.
cpr-none = Ainda não há curva: clique primeiro em pontos
cpr-cleared = Curva limpa
cpr-saved = Curva salva: { $name }
cpr-save-failed = Não foi possível salvar a curva: { $reason }
cpr-loaded = Curva carregada
# machine-drafted, needs native review
# $count: centrelines read from the file.
cpr-loaded-count = { $count ->
    [one] { $count } linha central carregada
    [many] { $count } de linhas centrais carregadas
   *[other] { $count } linhas centrais carregadas
}
# $count: centrelines found in files saved beside the volume.
cpr-sidecars-found = { $count ->
    [one] { $count } linha central salva encontrada
    [many] { $count } de linhas centrais salvas encontradas
   *[other] { $count } linhas centrais salvas encontradas
}
# A centreline without a name, by its place in the list.
cpr-centreline = Linha central { $number }
cpr-new = Nova linha central
cpr-rename = Renomear
cpr-name = Nome da linha central
cpr-delete = Excluir
# $name: the centreline's name.
cpr-delete-title = Excluir a linha central “{ $name }”?
cpr-delete-detail = Os pontos dela somem. Os arquivos salvos no disco continuam.
cpr-deleted = “{ $name }” excluída
cpr-load-failed = Não foi possível carregar a curva: { $reason }
cpr-load-not-json = o arquivo não é JSON
cpr-load-not-curves = o arquivo não é uma curva do Specular
cpr-load-empty = o arquivo não contém nenhuma curva de dois ou mais pontos
cpr-panoramic = Panorâmica
cpr-panoramic-slab = Panorâmica, espessura de { $thickness } mm
cpr-panoramic-thin = Panorâmica, fina

## The dental layout (Medical).
## machine-drafted, needs native review

dental-button = Odontológico
dental-not-for-diagnosis = Não destinado a diagnóstico nem a planejamento de tratamento
dental-nerve = Canal mandibular
dental-nerve-hint = Clique em pontos ao longo do canal mandibular na panorâmica ou em um corte transversal
dental-nerve-done = Traçado do canal mandibular desativado
dental-spacing = A cada { $spacing } mm
dental-section = Corte { $number }
dental-off = Abra primeiro o layout odontológico
dental-suggesting = Procurando a arcada dentária…
dental-suggested = Arcada sugerida: arraste seus pontos para ajustá-la
dental-no-arch = Nenhuma arcada encontrada: clique em pontos ao longo dela na vista axial
dental-vendor = TCFC odontológica { $vendor }

## Each segment as its own STL.
## machine-drafted, needs native review

seg-stl-export-button = Exportar segmentos
seg-stl-detail = Segmentos: { $count ->
        [one] { $count } triângulo
       *[other] { $count } triângulos
    }
seg-stl-detail-full = Segmentos: detalhe completo
seg-stl-none = Nenhum segmento para exportar
seg-stl-not-labels = este volume não é um mapa de rótulos
seg-stl-exporting = Construindo as superfícies dos segmentos…
seg-stl-saved = { $count ->
    [one] { $count } segmento salvo em { $folder }
    [many] { $count } segmentos salvos em { $folder }
   *[other] { $count } segmentos salvos em { $folder }
}
seg-stl-open = Não fechados: { $count }.
seg-stl-empty = Os segmentos estão vazios: nada foi salvo
seg-stl-failed = Não foi possível salvar os segmentos: { $reason }
# machine-drafted, needs native review
toolbar-mpr = Três planos (MPR)
toolbar-mpr-close = Fechar três planos (MPR)
settings-viewing-toolbar-items-mpr = Três planos (MPR)
