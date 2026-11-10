# machine-drafted, needs native review
## Revisão de modelos 3D: caminhar e voar, planos de corte e caixa de corte, vistas salvas,
## tópicos BCF (modo 3D). As teclas citadas são as padrão.

cheatsheet-review = Revisão

palette-mesh-walk = Caminhar
palette-mesh-fly = Voar
palette-mesh-views = Vistas salvas
palette-mesh-save-view = Salvar a vista
palette-mesh-section-plane = Adicionar um plano de corte
palette-mesh-section-box = Caixa de corte
palette-mesh-section-flip = Inverter o plano de corte
palette-mesh-section-clear = Remover todos os cortes
palette-mesh-measure-cut = Medir no corte
palette-mesh-bcf-topics = Tópicos BCF
# machine-drafted, needs native review
palette-mesh-model-checks = Verificações do modelo
# machine-drafted, needs native review
palette-mesh-models = Modelos na cena
# machine-drafted, needs native review
palette-mesh-add-model = Adicionar modelo…
# machine-drafted, needs native review
palette-mesh-add-model-folder = Adicionar modelos de uma pasta…

# Caminhar e voar: avisos e linhas de leitura, conforme $what. $eye: altura do olho acima do
# piso, com a unidade (só walk-line).
mesh-nav = { $what ->
    [walk-toast] Caminhando: W A S D para mover, arraste para olhar, Esc para parar
    [fly-toast] Voando: W A S D para mover, Q E para descer e subir, Esc para parar
    [orbit] Órbita
    [walk-line] Caminhando · olho a { $eye } do piso
    [walk-keys] W A S D move · arrastar olha · Shift mais rápido · Esc para
    [preparing] Paredes ainda não prontas: você as atravessa
    [fly-keys] W A S D move · Q E desce, sobe · arrastar olha · Shift mais rápido · Esc para
   *[fly-line] Voando
}

# Textos curtos de cortes, medição e vistas salvas, conforme $what.
bim-note = { $what ->
    [plane-added] Plano de corte adicionado: arraste a alça para movê-lo
    [box-on] Caixa de corte ativada: arraste a alça de uma face para movê-la
    [box-off] Caixa de corte desativada
    [cleared] Cortes removidos
    [measure-on] Clique em dois pontos do corte
    [measure-needs-cut] Adicione primeiro um plano de corte ou a caixa
    [measure-off-cut] Clique no corte
    [measure-hint] Medindo: clique em dois pontos do corte
    [ask-view-name] Nome da vista
    [ask-rename] Novo nome
    [views-empty] Nenhuma vista salva. Pressione N para salvar esta.
    [views-hints] Enter ir · N salvar · F2 renomear · Delete remover · P salvar como PNG
    [delete-detail] Só a vista salva é removida. O modelo não muda.
    [delete-ok] Remover
    [not-over-model] Escolha outro arquivo: este substituiria o modelo.
   *[view-name] Vista
}

# Textos curtos de BCF (BIM Collaboration Format), conforme $what.
bcf-note = { $what ->
    [title-none] Tópicos BCF
    [new-file] novo
    [empty] Nenhum tópico. Pressione O para abrir um arquivo BCF ou N para adicionar um tópico.
    [hints] Enter ir · ← → pontos de vista · N tópico · C comentário · S status · E título · O abrir · Ctrl+S exportar
    [ask-title] Título do tópico
    [ask-comment] Comentário
    [untitled] Sem título
    [topic-added] Tópico adicionado com esta vista
   *[comment-added] Comentário adicionado com esta vista
}

# $planes: quantos planos de corte.
bim-sections-line = { $planes ->
    [0] Nenhum plano de corte
    [one] { $planes } plano de corte
   *[other] { $planes } planos de corte
}
# $size: o tamanho da caixa de corte, com a unidade.
bim-box-line = caixa { $size }
# $count: o máximo de planos possível.
bim-planes-full = No máximo { $count } planos de corte
# Uma alça de arrasto, para leitores de tela. $n: o número dela.
bim-handle = { $kind ->
    [box] Face { $n } da caixa de corte
   *[plane] Plano de corte { $n }
}
# $length: o comprimento com a unidade.
bim-measure-line = Comprimento no corte: { $length }
bim-views-title = { $count ->
    [one] { $count } vista salva
   *[other] { $count } vistas salvas
}
# $name: o nome de uma vista.
bim-view-saved = “{ $name }” salva
bim-view-delete-title = Remover a vista “{ $name }”?
# $file: um arquivo gravado.
bim-saved-file = { $file } salvo
# $reason: por que falhou.
bim-failed = { $what ->
    [open] Não foi possível abrir o arquivo BCF: { $reason }
    [views] Vistas mantidas só nesta sessão: { $reason }
   *[save] Não foi possível salvar: { $reason }
}
# $count: tópicos; $file: o nome do arquivo.
bcf-title = { $count ->
    [one] { $count } tópico
   *[other] { $count } tópicos
} · { $file }
bcf-comments = { $count ->
    [one] { $count } comentário
   *[other] { $count } comentários
}
# $at: o ponto de vista mostrado; $count: os pontos de vista do tópico.
bcf-viewpoint-of = Ponto de vista { $at } de { $count }
bcf-opened = { $count ->
    [one] { $count } tópico aberto
   *[other] { $count } tópicos abertos
}

## Several IFC models in one scene (the Models list, Alt+Shift+M).

# The list's title. $count: models in the scene.
# machine-drafted, needs native review
fed-title = { $count ->
    [one] Modelos: 1
   *[other] Modelos: { $count }
}
# machine-drafted, needs native review
fed-note = { $what ->
    [not-ifc] Este arquivo não é um modelo IFC.
    [not-ifc-open] Abra primeiro um modelo IFC. Só modelos IFC podem ser combinados.
    [this-model] Este modelo
    [hints] A adicionar modelo · Shift+A adicionar pasta · Space mostrar ou ocultar · C colorir por modelo · Enter ir · Delete remover
    [origin-ok] Posicionar na origem
    [first-stays] O modelo aberto permanece. Abra outro arquivo para trocá-lo.
    [remove-detail] Ele sai da cena. O arquivo no disco permanece.
   *[remove-ok] Remover
}
# A model's row. $basis: how it was placed; $elements: its elements; $shown: shown or hidden.
# machine-drafted, needs native review
fed-row = { $basis } · { $elements ->
    [one] 1 elemento
   *[other] { $elements } elementos
} · { $shown ->
    [hidden] oculto
   *[shown] visível
}
# How a model was placed against the first. $crs: the coordinate system's name.
# machine-drafted, needs native review
fed-basis = { $basis ->
    [reference] modelo aberto
    [map-crs] por coordenadas de mapa ({ $crs })
    [map] por coordenadas de mapa
    [site] por latitude e longitude do terreno (aproximado)
    [project] por coordenadas compartilhadas do projeto
   *[origin] na origem (não alinhado)
}
# Why a model cannot be aligned. $first, $this: coordinate system names.
# machine-drafted, needs native review
fed-unaligned = { $why ->
    [crs] Os modelos usam sistemas de coordenadas diferentes: { $first } e { $this }.
    [only-first] O modelo aberto é georreferenciado, este não.
    [only-this] Este modelo é georreferenciado, o aberto não.
   *[broken] A conversão de mapa deste modelo não pode ser usada (sua escala é zero).
}
# The chosen model's placement. $distance: how far it moved; $angle: degrees turned.
# machine-drafted, needs native review
fed-placed = Movido { $distance }, girado { $angle }°
# machine-drafted, needs native review
fed-adding = { $count ->
    [one] Lendo 1 modelo…
   *[other] Lendo { $count } modelos…
}
# machine-drafted, needs native review
fed-ask-origin = Não é possível alinhar { $file }. Posicioná-lo na origem?
# machine-drafted, needs native review
fed-no-files = Nenhum arquivo IFC em { $folder }.
# machine-drafted, needs native review
fed-not-added = { $file } não foi adicionado: { $reason }
# machine-drafted, needs native review
fed-added = { $file } adicionado. Modelos na cena: { $count }.
# machine-drafted, needs native review
fed-remove-title = Remover { $file } da cena?
# machine-drafted, needs native review
fed-removed = { $file } removido da cena.
# machine-drafted, needs native review
fed-shown = { $file } { $shown ->
    [hidden] oculto
   *[shown] visível
}
# machine-drafted, needs native review
fed-colours = { $on ->
    [on] Colorir por modelo ativado
   *[off] Colorir por modelo desativado
}

## PMI (product manufacturing information) of a CAD model: dimensions, geometric tolerances,
## datums and the file's saved views (mesh mode).

# machine-drafted, needs native review
palette-mesh-pmi = PMI e vistas salvas
# $count: annotations in the model.
# machine-drafted, needs native review
pmi-title = { $count ->
    [one] 1 anotação
   *[other] { $count } anotações
}
# machine-drafted, needs native review
pmi-view = Vista salva
# machine-drafted, needs native review
pmi-view-all = mostra todas as anotações
# $count: annotations the saved view shows.
# machine-drafted, needs native review
pmi-view-shows = { $count ->
    [one] mostra 1 anotação
   *[other] mostra { $count } anotações
}
# $name: the saved view's name.
# machine-drafted, needs native review
pmi-view-shown = Mostrando a vista “{ $name }”. A mostra tudo.
# machine-drafted, needs native review
pmi-has-faces = faces destacadas
# Short texts, by $what.
# machine-drafted, needs native review
pmi-note = { $what ->
    [none] Nenhuma PMI neste modelo.
    [hints] ↑ ↓ escolher · Enter ir · A tudo · D como desenhado · Shift+A ocultar
    [hidden] Anotações ocultas
    [shown] Anotações visíveis
    [as-drawn] Anotações como o arquivo as desenha
    [as-labels] Anotações como rótulos
   *[all-shown] Todas as anotações visíveis
}
# What an annotation is, by $what.
# machine-drafted, needs native review
pmi-kind = { $what ->
    [dimension] Cota
    [tolerance] Tolerância geométrica
    [datum] Referência
    [target] Referência parcial
   *[graphic] Anotação desenhada
}
# machine-drafted, needs native review
pmi-dimension = { $what ->
    [linear] Distância
    [angular] Ângulo
    [diameter] Diâmetro
    [radius] Raio
    [length] Comprimento
    [thickness] Espessura
   *[other] Cota
}
# Geometric characteristics (ISO 1101), by $what.
# machine-drafted, needs native review
pmi-characteristic = { $what ->
    [straightness] Retilineidade
    [flatness] Planeza
    [circularity] Circularidade
    [cylindricity] Cilindricidade
    [line-profile] Forma de uma linha
    [surface-profile] Forma de uma superfície
    [angularity] Inclinação
    [perpendicularity] Perpendicularidade
    [parallelism] Paralelismo
    [position] Posição
    [concentricity] Concentricidade
    [coaxiality] Coaxialidade
    [symmetry] Simetria
    [circular-runout] Batimento circular
   *[total-runout] Batimento total
}
