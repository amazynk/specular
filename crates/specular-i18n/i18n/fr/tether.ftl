# Specular — tethered capture: a camera on USB, its shots downloaded as they are taken. Français. machine-drafted, needs native review

palette-tether = Prise de vue connectée…
palette-tether-capture = Prendre une photo (connectée)

tether-title = Prise de vue connectée
tether-close = Fermer la prise de vue connectée
tether-default-folder = Prise de vue connectée

## Cameras
tether-none = Aucun appareil trouvé.
tether-none-hint = Branchez l’appareil avec son câble USB et allumez-le. S’il a un mode USB, choisissez PTP ou connexion PC.
tether-look-again = Chercher à nouveau
tether-connect = Connecter
tether-next-camera = Appareil suivant
tether-connecting = Connexion…
tether-connected = { $camera } connecté
tether-disconnect = Déconnecter
tether-try-again = Réessayer
tether-gone = L’appareil a été déconnecté.
tether-not-connected = Aucun appareil connecté. Ouvrez d’abord la prise de vue connectée.

## Why a camera can't be opened
tether-permission = Cet utilisateur n’a pas le droit d’ouvrir le périphérique USB de l’appareil { $node }.
tether-permission-rule = Une règle udev l’autorise. En administrateur, ajoutez cette ligne à { $file } :
tether-permission-reload = Puis lancez « sudo udevadm control --reload » et rebranchez l’appareil.
tether-copy-rule = Copier la règle
tether-busy-other = Un autre programme utilise l’appareil.
tether-busy-named = L’appareil est utilisé par { $programs }.
tether-busy-hint = Fermez ce programme ou démontez l’appareil dans le gestionnaire de fichiers, puis réessayez.
tether-driver = Le pilote système de cet appareil ne laisse pas d’autres programmes l’utiliser.

## The camera
tether-battery = Batterie { PERCENT($percent) }
tether-card = { $card } : { $free } libres
tether-card-shots = { $card } : { $free } libres, { $shots } vues
tether-no-card = Pas de carte mémoire.
tether-no-settings = L’appareil n’indique pas ses réglages d’exposition.
tether-no-remote = Cet appareil ne peut pas être déclenché d’ici. Les vues prises avec son déclencheur sont téléchargées.
tether-capture = Déclencher
tether-step-down = { $setting } : diminuer ({ $value })
tether-step-up = { $setting } : augmenter ({ $value })

## Exposure settings
tether-prop-mode = Mode d’exposition
tether-prop-shutter = Vitesse
tether-prop-aperture = Ouverture
tether-prop-iso = ISO
tether-prop-bias = Correction d’exposition
tether-prop-white-balance = Balance des blancs
tether-auto = Automatique
tether-bulb = Pose B
tether-camera-value = Réglage de l’appareil { $value }
tether-wb-manual = Manuelle
tether-wb-one-push = Mesure ponctuelle
tether-wb-daylight = Lumière du jour
tether-wb-fluorescent = Fluorescent
tether-wb-tungsten = Tungstène
tether-wb-flash = Flash
tether-mode-manual = Manuel
tether-mode-program = Programme
tether-mode-aperture = Priorité ouverture
tether-mode-shutter = Priorité vitesse
tether-mode-creative = Créatif
tether-mode-action = Sport
tether-mode-portrait = Portrait

## Where shots go
tether-folder = Dossier
tether-names = Noms
tether-names-camera = Les noms de l’appareil
tether-arrival = Style Develop
tether-choose-folder = Dossier…
tether-choose-style = Style…
tether-downloading = Téléchargement… { PERCENT($percent) }
tether-saved = { $name } enregistré
tether-saved-count = { $count ->
    [one] { $count } vue enregistrée
   *[other] { $count } vues enregistrées
}

## Failures
tether-busy = L’appareil est occupé. Réessayez.
tether-card-full = La carte mémoire est pleine.
tether-set-refused = L’appareil n’a pas accepté ce réglage. Vérifiez la molette des modes.
tether-disk = Impossible d’enregistrer la vue : { $error }
tether-error = Erreur de l’appareil : { $error }

## Settings › Cull mode
settings-cull-tether-folder = Dossier de la prise de vue connectée
settings-cull-tether-folder-help = Où sont enregistrées les vues d’un appareil connecté par câble.
settings-cull-tether-folder-unset = Images › Prise de vue connectée
settings-cull-tether-rename = Noms des fichiers en prise de vue connectée
settings-cull-tether-rename-help = Nouveaux noms, avec les mêmes balises que Renommer dans Import. Vide garde les noms de l’appareil.
