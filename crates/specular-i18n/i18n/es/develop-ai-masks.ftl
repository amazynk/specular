# machine-drafted, needs native review
# Specular — Develop's AI masks: subject, sky and object-at-click rows, and the
# Masks feature in Settings › Local AI.

develop-mask-kind-subject = Sujeto
develop-mask-kind-sky = Cielo
develop-mask-kind-object = Objeto al hacer clic
# An AI mask row while its network runs. $kind: subject, sky, object.
develop-ai-computing = { $kind ->
        [subject] Buscando el sujeto…
        [sky] Buscando el cielo…
       *[object] Buscando el objeto…
    }
develop-ai-ready = Listo
# An object row with its points ($n; a box counts as two).
develop-ai-object-clicks = Puntos: { $n }
develop-ai-click-help =
    Haz clic en el objeto.
    Alt+clic en lo que no forma parte de él.
    Arrastra para un recuadro.
# The row's model is not installed. $model is its name ("IS-Net").
develop-ai-needs =
    Necesita el complemento { $model }.
    Hasta entonces no enmascara nada.
develop-ai-failed = No se pudo crear esta máscara: { $error }
develop-ai-failed-worker = El proceso de máscaras de IA se detuvo.
develop-ai-open-settings = Abrir IA local…
develop-ai-clear = Borrar puntos
develop-ai-too-many = Como máximo { $n } puntos (un recuadro cuenta como dos).
develop-ai-box-set = Recuadro definido
develop-ai-point-added = Punto añadido
develop-ai-point-removed = Punto añadido fuera del objeto
develop-ai-cleared = Puntos borrados
# Screen-reader names of the points on the picture. $n counts from 1.
develop-ai-point-on = Punto { $n }, en el objeto
develop-ai-point-off = Punto { $n }, fuera del objeto

## Settings › Local AI: the Masks feature
settings-ai-masks = Máscaras de IA
settings-ai-masks-help = Máscaras de sujeto, cielo y objeto en Revelar. El modelo de objetos viene con esta función; sujeto y cielo son complementos.
settings-ai-masks-size = Modelo
settings-ai-masks-size-help = EfficientSAM Ti es más rápido. EfficientSAM S sigue los bordes con más precisión.
settings-ai-masks-size-recommended = Predeterminado
settings-ai-masks-size-small = EfficientSAM Ti
settings-ai-masks-size-standard = EfficientSAM S
settings-ai-masks-size-large = Grande
