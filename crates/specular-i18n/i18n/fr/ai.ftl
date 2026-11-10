# Specular — Local AI pack (DESIGN.md "Local AI pack", §10). French. Machine-drafted, needs native review.

## Settings → Local AI
settings-section-ai = IA locale
settings-ai-pack = Pack d’IA locale
# machine-drafted, needs native review
settings-ai-pack-help = Des fonctions facultatives qui s’exécutent uniquement sur cet ordinateur. Rien de ce qu’elles trouvent ne le quitte. Choisissez un modèle pour chaque fonction ; rien n’est téléchargé tant que vous ne l’avez pas choisi.
# machine-drafted, needs native review
settings-ai-install-from-file = Installer depuis un fichier…
settings-ai-install-from-file-help = Installer des fichiers de modèle copiés sur cet ordinateur, pour les machines sans internet. Chaque fichier est vérifié avec la liste fournie avec { -app-name }.
settings-ai-install-from-folder = Installer depuis un dossier…
settings-ai-install-from-folder-help = La même chose, pour chaque fichier de modèle d’un dossier.
settings-ai-search = Recherche par description
settings-ai-search-help = Trouver des photos en les décrivant, par exemple « chien sur une plage », et mieux regrouper les prises similaires.
settings-ai-text = Texte dans les images
settings-ai-text-help = Lire le texte des photos et des numérisations, pour que la recherche text: les trouve.
settings-ai-culling = Aide au tri
# machine-drafted, needs native review
settings-ai-culling-help = Signaler les yeux fermés, le flou et une mauvaise exposition pour aider à choisir la meilleure photo d’une rafale.
settings-ai-people = Personnes
# machine-drafted, needs native review
settings-ai-people-help = Trouver les visages et les regrouper par personne, pour la recherche people:. Le regroupement reste désactivé tant que vous ne l’activez pas pour une bibliothèque. Les données de visage peuvent être supprimées pour une personne ou pour toutes à la fois.

# The pack's and a feature's state. $percent is formatted ("25%"); $done and $total are
# formatted numbers.
ai-state = { $state ->
        [none] Aucun modèle disponible dans cette version.
        [installed] Installé
        [indexing] Installé · { $done } images sur { $total } indexées
        [downloading] Téléchargement… { $percent }
        [failed] Échec du téléchargement
       *[available] Non installé
    }
ai-removing = Suppression…
# machine-drafted, needs native review
# Buttons of the Local AI page.
ai-button = { $action ->
        [download] Télécharger
        [cancel] Annuler
        [remove] Supprimer
        [forget] Oublier
        [faces] Supprimer toutes les données de visage
       *[pack] Supprimer le pack
    }
# The question before deleting.
ai-confirm = { $action ->
        [faces] Supprimer toutes les données de visage ? Tous les visages et toutes les personnes trouvés sont supprimés. Vos photos restent.
       *[pack] Supprimer le pack d’IA locale, ses modèles et tout ce qu’il a trouvé ? Vos photos ne sont pas modifiées.
    }

## Gallery search
gallery-query-needs-ai = { $field } : nécessite le pack d’IA locale (Paramètres › IA locale)
# machine-drafted, needs native review
gallery-query-ai-not-yet = { $field } : le pack d’IA locale ne peut pas encore rechercher cela

## Faces: People view, face boxes and culling flags
# machine-drafted, needs native review
# "Show faces" on the picture.
faces-shown = Visages affichés
# machine-drafted, needs native review
faces-hidden = Visages masqués
# machine-drafted, needs native review
# Beside a face box whose eyes look closed.
faces-eyes-closed = Yeux fermés
# machine-drafted, needs native review
# Culling flags on the cull mode strip.
cull-flag-eyes-closed = Yeux fermés
# machine-drafted, needs native review
cull-flag-eyes-closed-some = Yeux fermés : { $closed } sur { $faces } visages
# machine-drafted, needs native review
cull-flag-blurred = Floue
# machine-drafted, needs native review
cull-flag-dark = Trop sombre
# machine-drafted, needs native review
cull-flag-bright = Trop claire
# machine-drafted, needs native review
cull-flag-clipped = Hautes lumières ou ombres écrêtées
# machine-drafted, needs native review
# The People view.
people-title = Personnes
# machine-drafted, needs native review
people-open = Personnes
# machine-drafted, needs native review
people-done = Terminé
# machine-drafted, needs native review
people-back = Retour
# machine-drafted, needs native review
people-show-photos = Afficher les photos
# machine-drafted, needs native review
people-name = Nommer…
# machine-drafted, needs native review
people-name-prompt = Nommer cette personne
# machine-drafted, needs native review
people-faces-open = Visages
# machine-drafted, needs native review
people-faces-of = Visages de { $name }
# machine-drafted, needs native review
people-merge = Fusionner
# machine-drafted, needs native review
people-merged = { $count ->
        [one] { $count } personne fusionnée
       *[other] { $count } personnes fusionnées
    }
# machine-drafted, needs native review
people-merge-ask = Fusionner { $from } avec { $into } ?
# machine-drafted, needs native review
people-merge-ask-many = Fusionner { $count } personnes dans { $into } ?
# machine-drafted, needs native review
people-merge-ask-detail = Ses visages deviennent ceux de { $into }. Les photos sur le disque ne changent pas.
# machine-drafted, needs native review
people-not-this = Pas cette personne
# machine-drafted, needs native review
people-face-removed = Visage retiré de cette personne. La photo est conservée.
# machine-drafted, needs native review
# Someone not named yet.
people-unnamed = Personne #{ $id }
# machine-drafted, needs native review
people-face-count = { $count ->
        [one] { $count } visage
       *[other] { $count } visages
    }
# machine-drafted, needs native review
people-count = { $count ->
        [one] { $count } personne
       *[other] { $count } personnes
    }
# machine-drafted, needs native review
people-face-a11y = Visage { $index }
# machine-drafted, needs native review
people-loading = Chargement…
# machine-drafted, needs native review
people-not-installed = Personnes n’est pas installé. Téléchargez-le dans Paramètres › IA locale.
# machine-drafted, needs native review
people-grouping-off = Le regroupement des visages est désactivé pour cette bibliothèque. Activez « Regrouper les visages en personnes » dans Paramètres › IA locale.
# machine-drafted, needs native review
people-none = Aucune personne trouvée pour l’instant.

# machine-drafted, needs native review
# A feature's model: Default (the model Specular ships as the default, or the nearest one
# this computer can run) or a model by name, which stays that model on any computer. The
# page shows it only for a feature with more than one model; the dropdown labels Default
# with the model it means (ai-size-recommended-option) and names each model as the model
# list does. The option names below are the palette's and the command line's: model names
# are proper names, kept as they are.
settings-ai-search-size = Modèle
settings-ai-search-size-help = SigLIP 2 B/16 trouve le mieux les photos, dans de nombreuses langues. B/32 indexe environ quatre fois plus vite, pour les cartes graphiques plus lentes.
settings-ai-search-size-recommended = Par défaut
# machine-drafted, needs native review
settings-ai-search-size-small = SigLIP 2 B/32
settings-ai-search-size-standard = PE Core S16
settings-ai-search-size-large = SigLIP 2 B/16
settings-ai-text-size = Modèle
settings-ai-text-size-help = PP-OCRv6 small lit plus précisément ; tiny est plus rapide.
settings-ai-text-size-recommended = Par défaut
settings-ai-text-size-small = PP-OCRv6 tiny
settings-ai-text-size-standard = PP-OCRv6 small
settings-ai-text-size-large = Grand
# machine-drafted, needs native review
settings-ai-culling-size = Modèle
settings-ai-culling-size-help = Le modèle qui détecte les yeux fermés.
settings-ai-culling-size-recommended = Par défaut
settings-ai-culling-size-small = Petit
# machine-drafted, needs native review
settings-ai-culling-size-standard = Face Landmarker
settings-ai-culling-size-large = Grand
# machine-drafted, needs native review
settings-ai-people-size = Modèle
settings-ai-people-size-help = SFace est petit. AuraFace est plus grand, entraîné sur des données que son éditeur dit sous licence pour un usage commercial.
settings-ai-people-size-recommended = Par défaut
settings-ai-people-size-small = Petit
# machine-drafted, needs native review
settings-ai-people-size-standard = BlazeFace + SFace
settings-ai-people-size-large = BlazeFace + AuraFace
# A model size by name.
ai-tier = { $tier ->
        [small] Petit
        [large] Grand
       *[standard] Standard
    }
# machine-drafted, needs native review
# Default in the model dropdown, and the label of the "Back to default" row: with the model
# it means on this computer, "Default (SigLIP 2 B/16)". $size is the model's name.
ai-size-recommended-option = Par défaut ({ $size })
# Under the model setting: the default, or the nearest model this computer can run. $gpu is
# the graphics card's name.
ai-recommended-for = Par défaut pour votre { $gpu } : { $size }
ai-recommended = Par défaut pour cet ordinateur : { $size }
# Nothing fits: no size can run here.
ai-recommended-none = Aucune taille ne tient dans la mémoire de cet ordinateur.
# Under an installed feature: what its model runs on now. $gpu is the graphics card's name
# (used with "gpu" only). The CPU variants say why the graphics card is not used.
ai-runs-on = { $engine ->
        [gpu] Exécution : GPU ({ $gpu })
        [gpu-unnamed] Exécution : GPU
        [no-gpu] Exécution : CPU (aucun GPU trouvé)
        [differed] Exécution : CPU — le résultat du GPU différait de celui du CPU
        [failed] Exécution : CPU — le modèle a échoué sur le GPU
        [setting] Exécution : CPU (traitement réglé sur « CPU uniquement »)
        [checking] Exécution : CPU — vérification du GPU par rapport à lui
       *[cpu] Exécution : CPU
    }
# A size's row: its files on disk ($place "disk"), or what it would download, the memory it
# needs, and " · needs the GPU" only for a size that cannot run without one. $bytes and
# $memory are formatted sizes.
ai-size-detail = { $place ->
        [disk] { $bytes } sur le disque
       *[download] { $bytes } à télécharger
    } · { $memory } de mémoire{ $gpu ->
        [yes] { " · nécessite le GPU" }
       *[no] { "" }
    }
# machine-drafted, needs native review
# Under a size that may not run on this computer (it is still shown, and choosing it asks
# first). "gpu": it needs a graphics card this computer does not have or cannot use;
# "memory": $need is the memory it needs, $have what this computer has for it (formatted
# sizes, "6 GB").
ai-size-unfit = { $why ->
        [gpu] Nécessite un GPU, et cet ordinateur n’en a aucun qu’il puisse utiliser.
       *[memory] Nécessite { $need } de mémoire, plus que les { $have } que Specular autorise pour l’IA sur cet ordinateur.
    }
# Over the size setting while it names a size other than the one installed and in use
# ($current, $chosen: ai-tier). The rows below have the buttons to change it.
ai-size-not-in-use = { $current } est en service, pas { $chosen }.
# machine-drafted, needs native review
# Buttons of a size's row and of the size question: "use" chooses that size (one not
# downloaded; a downloaded one says Switch, ai-size-button), "back" returns the setting to
# Recommended, "anyway" keeps a size that may not run on this computer (ai-unfit-question).
# The screen reader adds the size's name to each.
ai-size-action = { $action ->
        [back] Revenir au modèle par défaut
        [anyway] Utiliser quand même
       *[use] Utiliser cette taille
    }
# The size the setting chose is not installed while another one is ($current, ai-tier).
ai-size-chosen-missing = Choisie, mais pas encore téléchargée : { $current } reste en service.
# What a size is now: from what is installed, never from the setting. "downloaded": its files
# are here but another size is in use; "missing": the chosen size, not downloaded.
ai-size-state = { $state ->
        [current] En service
        [downloaded] Téléchargée, pas en service
        [missing] Pas téléchargée
        [previous] En service jusqu’à ce que la nouvelle taille ait indexé votre bibliothèque
        [target] Téléchargement…
        [failed] Échec du téléchargement
       *[other] { "" }
    }
# A licence row's button (opens the licence's page).
ai-licence = Voir la licence
# Asked before a size change that downloads and re-indexes. $feature is the feature's name,
# $size the new size (ai-tier), $download a formatted size.
ai-switch-question = { $download ->
        [none] { $feature } : passer à { $size } ? Votre bibliothèque sera réindexée ; la taille actuelle reste en service jusque-là.
       *[other] { $feature } : passer à { $size } ? Cela télécharge { $download } et réindexe votre bibliothèque ; la taille actuelle reste en service jusque-là.
    }
# machine-drafted, needs native review
# A size that may not run on this computer, chosen for a feature not installed yet: asked
# before the setting keeps it (ai-size-unfit says why, on the next line). $size is ai-tier,
# $feature the feature's name.
ai-unfit-question = { $feature } : utiliser { $size } quand même ? Elle risque de ne pas fonctionner sur cet ordinateur, ou seulement très lentement.
# The recommendation moved: offered, never applied silently.
ai-suggest-question = { $feature } : { $size } convient désormais mieux à cet ordinateur. Changer ? { $download ->
        [none] Votre bibliothèque sera réindexée.
       *[other] Cela télécharge { $download } et réindexe votre bibliothèque.
    }
ai-suggest-toast = IA locale : une autre taille de modèle convient mieux à cet ordinateur (Paramètres › IA locale).
# A feature switching size: the new size ($size, ai-tier) indexes the library while the old one
# stays in use. $done and $total are formatted numbers.
ai-switching = Passage à { $size } · { $done } images sur { $total } indexées

## The model manager: what each model says besides its size and licence

# machine-drafted, needs native review
# A model or feature not installable yet.
ai-coming = Disponible dans une version ultérieure
# machine-drafted, needs native review
# What a model reads. $languages: multilingual, english, or a script id (OCR packs).
ai-choice-languages = { $languages ->
        [multilingual] Nombreuses langues
        [english] Anglais uniquement
        [cjk-latin] Chinois, japonais et alphabet latin
        [zh-latin] Chinois et alphabet latin
        [arabic] Arabe
        [hebrew] Hébreu
        [korean] Coréen
        [cyrillic] Cyrillique
        [greek] Grec
        [thai] Thaï
        [devanagari] Devanagari
        [tamil] Tamoul
        [telugu] Télougou
        [eslav] Russe, ukrainien et biélorusse
        [latin] Écriture latine (de nombreuses langues européennes)
       *[other] { $languages }
    }
# A hint about a model. $hint: faster, closed-eyes, small-faces.
ai-choice-hint = { $hint ->
        [faster] Plus rapide, pour les cartes graphiques plus lentes
        [closed-eyes] Détecte les yeux fermés
        [small-faces] Meilleur avec les petits visages
        [no-translate] Ne traduit pas en anglais
       *[other] { "" }
    }
# A model's training data. $source: the data as its publisher names it ($named is "no" when
# it is not named). $terms: whether the publisher states the data's terms ("stated"), does
# not ("unstated"), or the data is licensed for research only ("research").
ai-choice-data = { $terms ->
        [stated] Données d’entraînement : { $source }
        [research] Données d’entraînement : { $source } (licence pour la recherche uniquement)
       *[unstated] { $named ->
            [no] Données d’entraînement : non nommées, conditions non indiquées
           *[yes] Données d’entraînement : { $source }, conditions non indiquées
        }
    }
# Where a model downloads from. $host is a host name ("huggingface.co").
ai-choice-host = Téléchargé depuis { $host }
# A model still coming, in a feature's row. $size is formatted, $licence an SPDX name.
ai-choice-coming = { $name }: { $size }, { $licence }
# People: grouping faces into people, per library.
ai-grouping = Regrouper les visages en personnes
ai-grouping-help = Désactivé tant que vous ne l’activez pas pour cette bibliothèque. Le désactiver arrête le regroupement ; les visages déjà regroupés restent jusqu’à ce que vous les supprimiez.
ai-grouping-state = { $on ->
        [yes] Activé dans cette bibliothèque
       *[no] Désactivé
    }
ai-grouping-button = { $on ->
        [yes] Désactiver
       *[no] Activer
    }
# machine-drafted, needs native review
# An add-on (a script's text recogniser) before its feature is downloaded. $feature is the
# feature's name ("Text in photos").
ai-addon-first = Téléchargez d’abord { $feature }.
# machine-drafted, needs native review
# Asked before an add-on is removed. $name is the model's name ("PP-OCRv5 Arabic").
ai-addon-remove-title = Supprimer { $name } ?
# machine-drafted, needs native review
ai-addon-remove-detail = Ses fichiers de modèle sont supprimés. Vos photos et le texte déjà trouvé restent.
# machine-drafted, needs native review
# Text: reading the text in this library's photos, per library.
ai-text-indexing = Lire le texte des photos de cette bibliothèque
# machine-drafted, needs native review
ai-text-indexing-help = Désactivé tant que vous ne l’activez pas pour cette bibliothèque. Ensuite, text: dans la recherche de la Bibliothèque trouve les photos d’après les mots qu’elles contiennent.
# machine-drafted, needs native review
ai-text-indexing-state = { $on ->
        [yes] Activé dans cette bibliothèque
       *[no] Désactivé
    }

## Installation depuis un fichier

# machine-drafted, needs native review
# After model files were copied in.
ai-sideload-ready = { $count ->
        [one] { $count } fichier de modèle copié. Appuyez sur « Télécharger » pour l’installer.
        [many] { $count } fichiers de modèle copiés. Appuyez sur « Télécharger » pour les installer.
       *[other] { $count } fichiers de modèle copiés. Appuyez sur « Télécharger » pour les installer.
    }
ai-sideload-none = Aucun fichier de modèle trouvé.
# $file is a file name, or several separated by commas.
ai-sideload-unknown = { $file } n’est pas un modèle connu de { -app-name }. Il n’a pas été copié.
ai-sideload-failed = Impossible de copier { $file } : { $reason }
# Asked before using a file whose SHA-256 isn't in the shipped list. $model is the model it
# would be used as ("clip-vit-b32-1").
ai-unverified-title = Utiliser un modèle non vérifié ?
ai-unverified-detail = { $file } ne figure pas dans la liste des modèles fournis avec { -app-name } et ne peut donc pas être vérifié. Il sera utilisé comme { $model }.
ai-unverified-use = Utiliser le modèle non vérifié

# machine-drafted, needs native review
ai-failed-internal = Un problème est survenu dans { -app-name }. Le journal contient les détails.
# The buttons under a model-size question: switch, or keep the size in use.
ai-size-button = { $action ->
        [switch] Changer
       *[keep] Garder la taille actuelle
    }
