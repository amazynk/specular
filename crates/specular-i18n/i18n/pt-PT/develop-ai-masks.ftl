# machine-drafted, needs native review
# Specular — Develop's AI masks: subject, sky and object-at-click rows, and the
# Masks feature in Settings › Local AI.

develop-mask-kind-subject = Motivo
develop-mask-kind-sky = Céu
develop-mask-kind-object = Objeto ao clicar
# An AI mask row while its network runs. $kind: subject, sky, object.
develop-ai-computing = { $kind ->
        [subject] A procurar o motivo…
        [sky] A procurar o céu…
       *[object] A procurar o objeto…
    }
develop-ai-ready = Pronto
# An object row with its points ($n; a box counts as two).
develop-ai-object-clicks = Pontos: { $n }
develop-ai-click-help =
    Clique no objeto.
    Alt+clique no que não faz parte dele.
    Arraste para um retângulo.
# The row's model is not installed. $model is its name ("IS-Net").
develop-ai-needs =
    Precisa do suplemento { $model }.
    Até lá não mascara nada.
develop-ai-failed = Não foi possível criar esta máscara: { $error }
develop-ai-failed-worker = O processo de máscaras de IA parou.
develop-ai-open-settings = Abrir IA local…
develop-ai-clear = Limpar pontos
develop-ai-too-many = No máximo { $n } pontos (um retângulo conta como dois).
develop-ai-box-set = Retângulo definido
develop-ai-point-added = Ponto adicionado
develop-ai-point-removed = Ponto adicionado fora do objeto
develop-ai-cleared = Pontos limpos
# Screen-reader names of the points on the picture. $n counts from 1.
develop-ai-point-on = Ponto { $n }, no objeto
develop-ai-point-off = Ponto { $n }, fora do objeto

## Settings › Local AI: the Masks feature
settings-ai-masks = Máscaras de IA
settings-ai-masks-help = Máscaras de motivo, céu e objeto em Revelar. O modelo de objetos vem com esta funcionalidade; motivo e céu são suplementos.
settings-ai-masks-size = Modelo
settings-ai-masks-size-help = O EfficientSAM Ti é mais rápido. O EfficientSAM S segue as arestas com mais precisão.
settings-ai-masks-size-recommended = Predefinição
settings-ai-masks-size-small = EfficientSAM Ti
settings-ai-masks-size-standard = EfficientSAM S
settings-ai-masks-size-large = Grande
