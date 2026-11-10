# machine-drafted, needs native review
## Astronomy interop: DS9 regions, plate solving, SAMP, HiPS sky surveys.

palette-science-load-regions = Superposer des régions DS9…
palette-science-load-regions-help = Dessiner sur l’image les formes d’un fichier de régions DS9 (.reg).
palette-science-export-regions = Enregistrer les régions au format DS9…
palette-science-export-regions-help = Enregistrer les mesures et les régions superposées dans un fichier de régions DS9.
palette-science-edit-regions = Modifier les régions comme mesures
palette-science-edit-regions-help = Transformer les régions superposées en mesures que l’on peut déplacer et modifier.
palette-science-clear-regions = Retirer les régions superposées
palette-science-clear-regions-help = Retirer les régions DS9 de l’image.
palette-science-plate-solve = Résolution astrométrique
palette-science-plate-solve-help = Trouver les coordonnées célestes de l’image avec ASTAP ou astrometry.net. Relancer pour arrêter.
palette-science-save-plate-solution = Enregistrer la solution astrométrique
palette-science-save-plate-solution-help = Enregistrer les coordonnées célestes trouvées à côté de l’image, dans un fichier .wcs.
palette-science-sky-survey = Relevé du ciel en arrière-plan
palette-science-sky-survey-help = Afficher le relevé du ciel suivant autour de l’image, ou aucun.
palette-science-samp-send-image = Envoyer l’image aux applications SAMP
palette-science-samp-send-image-help = Ouvrir cette image dans DS9, Aladin et les autres applications du hub SAMP.
palette-science-samp-send-position = Envoyer la position aux applications SAMP
palette-science-samp-send-position-help = Pointer les autres applications vers la position céleste sous le pointeur.

disabled-samp-off = Se connecter d’abord à un hub SAMP (Paramètres › Intégration).

settings-integration-samp = Se connecter à un hub SAMP
settings-integration-samp-help = Échanger des images, des tables et des positions célestes avec DS9, TOPCAT, Aladin et d’autres applications d’astronomie. Seul un hub sur cet ordinateur est utilisé.
settings-integration-astap-path = Programme ASTAP
settings-integration-astap-path-help = Le solveur astrométrique ASTAP, s’il n’est pas trouvé automatiquement.
settings-integration-solve-field-path = Programme astrometry.net
settings-integration-solve-field-path-help = Le programme solve-field d’astrometry.net, s’il n’est pas trouvé automatiquement.

astro-regions-filter = Régions DS9
astro-regions-loaded = { $name } : { $count ->
        [one] { $count } région.
       *[other] { $count } régions.
    }
astro-regions-refused = { $count ->
        [one] { $count } ligne non lue.
       *[other] { $count } lignes non lues.
    } Première : ligne { $line }, { $reason }.
astro-regions-approximate = { $count ->
        [one] { $count } panda dessiné par ses bords intérieur et extérieur.
       *[other] { $count } pandas dessinés par leurs bords intérieur et extérieur.
    }
astro-regions-none-here = { $name } : aucune région ne tombe sur cette image.
astro-regions-failed = Impossible de lire { $name } : { $reason }
astro-regions-cleared = Régions retirées.
astro-regions-nothing = Aucune région ni mesure à enregistrer.
astro-regions-saved = { $name } enregistré.
astro-regions-skipped = { $count ->
        [one] { $count } forme hors des coordonnées célestes a été omise.
       *[other] { $count } formes hors des coordonnées célestes ont été omises.
    }
astro-regions-save-failed = Impossible d’enregistrer les régions : { $reason }
astro-regions-edited = { $count ->
        [one] { $count } région est maintenant une mesure.
       *[other] { $count } régions sont maintenant des mesures.
    }
astro-regions-no-wcs = Les régions célestes demandent une image avec des coordonnées célestes.

astro-solve-no-solver = Aucun solveur astrométrique trouvé. Installer ASTAP ou astrometry.net, ou en choisir un dans Paramètres › Intégration.
astro-solve-running = Résolution avec { $solver }… { $seconds } s
astro-solve-stop = Arrêter
astro-solve-stopped = Résolution astrométrique arrêtée.
astro-solve-done = Résolu : { $ra }, { $dec }, { $scale } par pixel.
astro-solve-failed = Échec de la résolution astrométrique : { $reason }
astro-solve-not-local = Seules les images d’un dossier sur cet ordinateur peuvent être résolues.
astro-solve-none = Pas encore de solution astrométrique pour cette image.
astro-solve-saved = { $name } enregistré. L’image elle-même est inchangée.
astro-solve-sidecar = Coordonnées célestes issues de { $name }.

astro-samp-connected = Connecté au hub SAMP.
astro-samp-no-hub = SAMP : { $reason }. Nouvel essai au démarrage d’un hub.
astro-samp-lost = Le hub SAMP s’est arrêté. Nouvel essai au démarrage d’un hub.
astro-samp-sent = { $count ->
        [one] Envoyé à { $count } application.
       *[other] Envoyé à { $count } applications.
    }
astro-samp-nobody = Aucune application du hub n’écoute ce message.
astro-samp-failed = SAMP : { $reason }
astro-samp-refused = SAMP : { $url } n’est pas sur cet ordinateur ; il n’est donc pas ouvert.
astro-samp-pointed = Pointé vers { $ra }, { $dec }.
astro-samp-off-image = { $ra }, { $dec } n’est pas sur cette image.
astro-samp-no-table = SAMP : cette table n’est pas superposée ici.
astro-samp-rows = { $count ->
        [one] { $count } source du catalogue mise en évidence.
       *[other] { $count } sources du catalogue mises en évidence.
    }

astro-survey-off = Relevé du ciel désactivé.
astro-survey-on = Relevé du ciel : { $name }. { $credit }
astro-survey-failed = Impossible d’afficher { $name } : { $reason }
astro-survey-credit = { $name } : { $credit }

net-what-sky-survey = Tuiles de relevé du ciel de { $host }
