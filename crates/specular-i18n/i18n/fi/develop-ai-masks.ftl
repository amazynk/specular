# machine-drafted, needs native review
# Specular — Develop's AI masks: subject, sky and object-at-click rows, and the
# Masks feature in Settings › Local AI.

develop-mask-kind-subject = Kohde
develop-mask-kind-sky = Taivas
develop-mask-kind-object = Objekti napsautuksella
# An AI mask row while its network runs. $kind: subject, sky, object.
develop-ai-computing = { $kind ->
        [subject] Etsitään kohdetta…
        [sky] Etsitään taivasta…
       *[object] Etsitään objektia…
    }
develop-ai-ready = Valmis
# An object row with its points ($n; a box counts as two).
develop-ai-object-clicks = Pisteitä: { $n }
develop-ai-click-help =
    Napsauta objektia.
    Alt+napsauta sitä, mikä ei kuulu siihen.
    Vedä kehys.
# The row's model is not installed. $model is its name ("IS-Net").
develop-ai-needs =
    Tarvitsee lisäosan { $model }.
    Ei peitä mitään siihen asti.
develop-ai-failed = Maskia ei voitu tehdä: { $error }
develop-ai-failed-worker = Tekoälymaskien prosessi pysähtyi.
develop-ai-open-settings = Avaa paikallinen tekoäly…
develop-ai-clear = Tyhjennä pisteet
develop-ai-too-many = Enintään { $n } pistettä (kehys lasketaan kahdeksi).
develop-ai-box-set = Kehys asetettu
develop-ai-point-added = Piste lisätty
develop-ai-point-removed = Piste lisätty objektin ulkopuolelle
develop-ai-cleared = Pisteet tyhjennetty
# Screen-reader names of the points on the picture. $n counts from 1.
develop-ai-point-on = Piste { $n }, objektilla
develop-ai-point-off = Piste { $n }, objektin ulkopuolella

## Settings › Local AI: the Masks feature
settings-ai-masks = Tekoälymaskit
settings-ai-masks-help = Kohde-, taivas- ja objektimaskit Kehitä-tilassa. Objektimalli tulee ominaisuuden mukana; kohde ja taivas ovat lisäosia.
settings-ai-masks-size = Malli
settings-ai-masks-size-help = EfficientSAM Ti on nopeampi. EfficientSAM S seuraa reunoja tarkemmin.
settings-ai-masks-size-recommended = Oletus
settings-ai-masks-size-small = EfficientSAM Ti
settings-ai-masks-size-standard = EfficientSAM S
settings-ai-masks-size-large = Suuri
