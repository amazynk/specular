# machine-drafted, needs native review

## Model checks on IFC building models (mesh mode, Alt+Q): IDS, clashes, changes between
## versions, quantities. IFC class names (IfcWall), property set names and values are data
## and stay as the file has them. Keys named here are the defaults.

# The panel's tabs.
checks-tab = { $tab ->
    [ids] IDS
    [clashes] Conflitos
    [changes] Alterações
   *[quantities] Quantitativos
}
# $tab: the tab's name.
checks-title = Verificações do modelo · { $tab }

# Short texts, by $what.
checks-note = { $what ->
    [not-ifc] As verificações do modelo precisam de um modelo IFC.
    [reading] Lendo as informações do modelo…
    [ids-empty] Nenhum arquivo IDS. Pressione O para abrir um.
    [ids-checking] Verificando…
    [ids-no-failures] Nada falha.
    [ids-hints] Enter falhas, ir · Backspace voltar · O abrir · P propriedades · Ctrl+S BCF · Ctrl+E CSV · Tab próximo
    [unnamed] Especificação sem nome
    [clash-running] Procurando conflitos…
    [clash-empty] Pressione R para procurar conflitos.
    [clash-none] Nenhum conflito.
    [clashes-hints] R executar · A B conjuntos · M físico ou folga · + − distância · Enter ir · S status · Ctrl+S BCF · Ctrl+E CSV · Tab próximo
    [changes-reading] Lendo a outra versão…
    [changes-empty] Pressione O para escolher outra versão deste modelo.
    [changes-none] Nenhuma alteração.
    [changes-hints] O outra versão · F filtro · Enter ir · P propriedades · Ctrl+S BCF · Ctrl+E CSV · Tab próximo
    [quantities-hints] G agrupar · Enter ir · P propriedades · Ctrl+E CSV · Ctrl+S XLSX · Tab próximo
   *[not-drawn] Este elemento não tem geometria para mostrar.
}

# What a requirement is about. $name: an attribute name, an IFC class or a pattern.
checks-facet = { $facet ->
    [entity] Classe
    [attribute] Atributo { $name }
    [classification] Classificação
    [material] Material
   *[partof] Parte de { $name }
}

# Why an element fails a requirement. $what: what the requirement is about; $actual: what
# the element has.
checks-reason = { $why ->
    [class] A classe é { $actual }
    [predefined] O tipo predefinido é { $actual }
    [missing] Falta { $what }
    [empty] { $what }: vazio
    [value] { $what } é { $actual }: não permitido
    [datatype] { $what } tem o tipo de dado { $actual }
    [system] { $what }: o sistema é { $actual }
    [whole] { $what }: parte de { $actual }
    [prohibited] { $what } não pode estar presente
   *[unsupported] { $what } não pode ser verificado
}

# Shown for a missing value.
checks-none = —

# A fact about the chosen element. $value: its value.
checks-prop = { $what ->
    [storey] Pavimento: { $value }
    [type] Tipo: { $value }
    [material] Material: { $value }
    [container] Contêiner: { $value }
   *[classification] Classificação: { $value }
}

# $file: the IDS file; $ok: specifications that hold; $count: all of them.
checks-ids-summary = { $file }: { $ok } de { $count } atendidas

# A specification's result. $applicable: objects it applies to; $failed: those failing.
checks-spec = { $state ->
    [pass] ✓ { $applicable } passam
    [fail] ✗ { $failed } de { $applicable } falham
    [none] ✗ não se aplica a nada
    [none-ok] ✓ não se aplica a nada
    [prohibited] ✗ { $applicable } que não devem existir
   *[prohibited-ok] ✓ nenhum que não deva existir
}

# A side of a clash test. $name: an IFC class or a storey's name.
checks-set = { $kind ->
    [all] todos os elementos
    [class] { $name }
   *[storey] pavimento { $name }
}
# $a, $b: the two sides; $distance: the tolerance or clearance with its unit.
checks-clash-setup = A: { $a } · B: { $b } · { $mode ->
    [hard] físico, mais profundo que { $distance }
   *[clearance] mais perto que { $distance }
}
# $count: clashes found; $new: new ones; $resolved: earlier ones no longer found; $seconds: time taken.
checks-clash-count = { $count ->
    [one] { $count } conflito
    [many] { $count } conflitos
   *[other] { $count } conflitos
}, { $new } novos, { $resolved } resolvidos · { $seconds } s
# One clash. $depth: the depth or distance with its unit; $status: its status.
checks-clash = { $kind ->
    [hard] físico { $depth }
   *[clearance] folga { $depth }
} · { $status }
checks-clash-kind = { $kind ->
    [hard] Físico
   *[clearance] Folga
}
checks-status = { $status ->
    [new] novo
    [active] ativo
   *[resolved] resolvido
}
# A BCF topic's title for a clash. $a, $b: the two elements.
checks-clash-topic = Conflito: { $a } × { $b }
checks-clash-done = { $count ->
    [0] Nenhum conflito
    [one] { $count } conflito
    [many] { $count } conflitos
   *[other] { $count } conflitos
}

# $file: the other version; counts of added, removed and changed elements; $filter: what is listed.
checks-changes-summary = { $file }: { $added } adicionados · { $removed } removidos · { $changed } alterados{ $filter ->
    [added] {" "}· mostrando adicionados
    [removed] {" "}· mostrando removidos
    [changed] {" "}· mostrando alterados
   *[all] {""}
}
checks-change = { $kind ->
    [added] Adicionado
    [removed] Removido
   *[geometry] geometria
}
checks-change-fields = { $count ->
    [one] { $count } campo
    [many] { $count } campos
   *[other] { $count } campos
}
# A field that changed. $name: the field; $old, $new: its values.
checks-field = { $name }: { $old } → { $new }
# $distance: how far it moved, with its unit.
checks-moved = Movido { $distance }

# $by: the grouping; $count: elements; $area, $volume: totals with units.
checks-qto-summary = { $count ->
        [one] { $count } elemento
       *[other] { $count } elementos
    } · { $area } · { $volume } · { $by ->
    [element] por elemento
    [class] por classe
    [storey] por pavimento
   *[type] por tipo
}
# Where quantities came from.
checks-source = { $source ->
    [file] do arquivo
    [computed] calculados
   *[mixed] alguns calculados
}
# A sheet of the quantities workbook.
checks-group = { $by ->
    [element] Elementos
    [class] Por classe
    [storey] Por pavimento
   *[type] Por tipo
}

# Column headers of exported tables.
checks-col = { $col ->
    [spec] Especificação
    [guid] GlobalId
    [class] Classe
    [name] Nome
    [reason] Motivo
    [a] A GlobalId
    [a-name] A
    [b] B GlobalId
    [b-name] B
    [kind] Tipo de conflito
    [depth] Profundidade ou distância (m)
    [status] Status
    [x] X (m)
    [y] Y (m)
    [z] Z (m)
    [change] Alteração
    [field] Campo
    [old] Antigo
    [new] Novo
    [type] Tipo
    [container] Contêiner
    [material] Material
    [classification] Classificação
    [storey] Pavimento
    [count] Quantidade
    [length] Comprimento (m)
    [area] Área (m²)
    [volume] Volume (m³)
   *[source] Origem
}

# $reason: why, in plain words.
checks-error = Não é possível: { $reason }
# A results summary for screen readers. $failed: specifications that do not hold.
checks-ids-done = { $failed ->
    [0] Verificado: todas as especificações são atendidas
    [one] Verificado: { $failed } especificação não é atendida
    [many] Verificado: { $failed } especificações não são atendidas
   *[other] Verificado: { $failed } especificações não são atendidas
}
# $file: the file written.
checks-saved = { $file } salvo

# Several models in one scene. An element or clash set and the model file it is from.
# machine-drafted, needs native review
checks-in-model = { $element } ({ $model })
# A clash set taken from every model of the scene.
# machine-drafted, needs native review
checks-every-model = todos os modelos
# The model IDS, changes and quantities are about. $model: its file name.
# machine-drafted, needs native review
checks-model = Modelo: { $model }
# A tab's keys in a scene of several models, by $tab (ids, clashes, changes, quantities).
# machine-drafted, needs native review
checks-hints-models = { $tab ->
    [ids] M modelo · Enter falhas, ir · Backspace voltar · O abrir · P propriedades · Ctrl+S BCF · Ctrl+E CSV · Tab próximo
    [clashes] R executar · A B conjuntos · G H seus modelos · M físico ou folga · + − distância · Enter ir · S status · Ctrl+S BCF · Ctrl+E CSV · Tab próximo
    [changes] M modelo · O outra versão · F filtro · Enter ir · P propriedades · Ctrl+S BCF · Ctrl+E CSV · Tab próximo
   *[quantities] M modelo · G agrupar · Enter ir · P propriedades · Ctrl+E CSV · Ctrl+S XLSX · Tab próximo
}
