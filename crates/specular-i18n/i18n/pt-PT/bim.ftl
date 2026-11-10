# machine-drafted, needs native review
## Revisão de modelos 3D: caminhar e voar, planos de corte e caixa de corte, vistas guardadas,
## tópicos BCF (modo 3D). As teclas indicadas são as predefinidas.

cheatsheet-review = Revisão

palette-mesh-walk = Caminhar
palette-mesh-fly = Voar
palette-mesh-views = Vistas guardadas
palette-mesh-save-view = Guardar a vista
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
# chão, com a unidade (só walk-line).
mesh-nav = { $what ->
    [walk-toast] A caminhar: W A S D para mover, arraste para olhar, Esc para parar
    [fly-toast] A voar: W A S D para mover, Q E para descer e subir, Esc para parar
    [orbit] Órbita
    [walk-line] A caminhar · olho a { $eye } do chão
    [walk-keys] W A S D move · arrastar olha · Shift mais rápido · Esc para
    [preparing] Paredes ainda não prontas: atravessa-as
    [fly-keys] W A S D move · Q E desce, sobe · arrastar olha · Shift mais rápido · Esc para
   *[fly-line] A voar
}

# Textos curtos de cortes, medição e vistas guardadas, conforme $what.
bim-note = { $what ->
    [plane-added] Plano de corte adicionado: arraste a pega para o mover
    [box-on] Caixa de corte ativada: arraste a pega de uma face para a mover
    [box-off] Caixa de corte desativada
    [cleared] Cortes removidos
    [measure-on] Clique em dois pontos do corte
    [measure-needs-cut] Adicione primeiro um plano de corte ou a caixa
    [measure-off-cut] Clique no corte
    [measure-hint] A medir: clique em dois pontos do corte
    [ask-view-name] Nome da vista
    [ask-rename] Novo nome
    [views-empty] Nenhuma vista guardada. Prima N para guardar esta.
    [views-hints] Enter ir · N guardar · F2 mudar o nome · Delete remover · P guardar como PNG
    [delete-detail] Só a vista guardada é removida. O modelo não é alterado.
    [delete-ok] Remover
    [not-over-model] Escolha outro ficheiro: este substituiria o modelo.
   *[view-name] Vista
}

# Textos curtos de BCF (BIM Collaboration Format), conforme $what.
bcf-note = { $what ->
    [title-none] Tópicos BCF
    [new-file] novo
    [empty] Nenhum tópico. Prima O para abrir um ficheiro BCF ou N para adicionar um tópico.
    [hints] Enter ir · ← → pontos de vista · N tópico · C comentário · S estado · E título · O abrir · Ctrl+S exportar
    [ask-title] Título do tópico
    [ask-comment] Comentário
    [untitled] Sem título
    [topic-added] Tópico adicionado com esta vista
   *[comment-added] Comentário adicionado com esta vista
}

# $planes: quantos planos de corte.
bim-sections-line = { $planes ->
    [0] Nenhum plano de corte
    [one] 1 plano de corte
   *[other] { $planes } planos de corte
}
# $size: o tamanho da caixa de corte, com a unidade.
bim-box-line = caixa { $size }
# $count: o máximo de planos possível.
bim-planes-full = No máximo { $count } planos de corte
# Uma pega de arrasto, para leitores de ecrã. $n: o seu número.
bim-handle = { $kind ->
    [box] Face { $n } da caixa de corte
   *[plane] Plano de corte { $n }
}
# $length: o comprimento com a unidade.
bim-measure-line = Comprimento no corte: { $length }
bim-views-title = { $count ->
    [one] 1 vista guardada
   *[other] { $count } vistas guardadas
}
# $name: o nome de uma vista.
bim-view-saved = «{ $name }» guardada
bim-view-delete-title = Remover a vista «{ $name }»?
# $file: um ficheiro escrito.
bim-saved-file = { $file } guardado
# $reason: porque falhou.
bim-failed = { $what ->
    [open] Não foi possível abrir o ficheiro BCF: { $reason }
    [views] Vistas mantidas só nesta sessão: { $reason }
   *[save] Não foi possível guardar: { $reason }
}
# $count: tópicos; $file: o nome do ficheiro.
bcf-title = { $count ->
    [one] 1 tópico
   *[other] { $count } tópicos
} · { $file }
bcf-comments = { $count ->
    [one] 1 comentário
   *[other] { $count } comentários
}
# $at: o ponto de vista mostrado; $count: os pontos de vista do tópico.
bcf-viewpoint-of = Ponto de vista { $at } de { $count }
bcf-opened = { $count ->
    [one] 1 tópico aberto
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
    [not-ifc] Este ficheiro não é um modelo IFC.
    [not-ifc-open] Abra primeiro um modelo IFC. Só é possível combinar modelos IFC.
    [this-model] Este modelo
    [hints] A adicionar modelo · Shift+A adicionar pasta · Space mostrar ou ocultar · C colorir por modelo · Enter ir · Delete remover
    [origin-ok] Posicionar na origem
    [first-stays] O modelo aberto mantém-se. Abra outro ficheiro para o mudar.
    [remove-detail] Sai da cena. O ficheiro no disco mantém-se.
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
    [project] por coordenadas partilhadas do projeto
   *[origin] na origem (não alinhado)
}
# Why a model cannot be aligned. $first, $this: coordinate system names.
# machine-drafted, needs native review
fed-unaligned = { $why ->
    [crs] Os modelos usam sistemas de coordenadas diferentes: { $first } e { $this }.
    [only-first] O modelo aberto está georreferenciado, este não.
    [only-this] Este modelo está georreferenciado, o aberto não.
   *[broken] A conversão de mapa deste modelo não pode ser usada (a sua escala é zero).
}
# The chosen model's placement. $distance: how far it moved; $angle: degrees turned.
# machine-drafted, needs native review
fed-placed = Deslocado { $distance }, rodado { $angle }°
# machine-drafted, needs native review
fed-adding = { $count ->
    [one] A ler 1 modelo…
   *[other] A ler { $count } modelos…
}
# machine-drafted, needs native review
fed-ask-origin = Não é possível alinhar { $file }. Posicioná-lo na origem?
# machine-drafted, needs native review
fed-no-files = Nenhum ficheiro IFC em { $folder }.
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
palette-mesh-pmi = PMI e vistas guardadas
# $count: annotations in the model.
# machine-drafted, needs native review
pmi-title = { $count ->
    [one] 1 anotação
   *[other] { $count } anotações
}
# machine-drafted, needs native review
pmi-view = Vista guardada
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
pmi-view-shown = Vista «{ $name }» visível. A mostra tudo.
# machine-drafted, needs native review
pmi-has-faces = faces destacadas
# Short texts, by $what.
# machine-drafted, needs native review
pmi-note = { $what ->
    [none] Nenhuma PMI neste modelo.
    [hints] ↑ ↓ escolher · Enter ir · A tudo · D como desenhado · Shift+A ocultar
    [hidden] Anotações ocultas
    [shown] Anotações visíveis
    [as-drawn] Anotações como o ficheiro as desenha
    [as-labels] Anotações como etiquetas
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
