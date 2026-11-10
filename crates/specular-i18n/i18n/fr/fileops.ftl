# machine-drafted, needs native review
# Opérations sur les fichiers : renommer, renommer par lot, copier et déplacer vers un dossier,
# rotation enregistrée dans le fichier, annuler et rétablir, fond d’écran, partager, envoyer par
# e-mail, exporter et importer les paramètres.

## Palette, aide-mémoire et menus

palette-undo = Annuler
palette-undo-help = Annuler la dernière modification de fichiers : mise à la corbeille, changement de nom, copie ou déplacement, rotation.
palette-redo = Rétablir
palette-redo-help = Refaire ce que la dernière annulation a défait.
palette-rename = Renommer…
palette-rename-help = Donner un nouveau nom au fichier ; ses fichiers annexes et son partenaire RAW+JPEG suivent.
palette-batch-rename = Renommer des fichiers…
palette-batch-rename-help = Renommer les fichiers sélectionnés, ou tout le dossier, selon un modèle, avec aperçu.
palette-copy-to-folder = Copier vers un dossier…
palette-copy-to-folder-help = Copier le fichier ou la sélection dans un autre dossier.
palette-move-to-folder = Déplacer vers un dossier…
palette-move-to-folder-help = Déplacer le fichier ou la sélection dans un autre dossier.
palette-rotate-file-left = Faire pivoter le fichier à gauche
palette-rotate-file-left-help = Tourner le fichier lui-même d’un quart vers la gauche, sans perte quand le format le permet.
palette-rotate-file-right = Faire pivoter le fichier à droite
palette-rotate-file-right-help = Tourner le fichier lui-même d’un quart vers la droite, sans perte quand le format le permet.
palette-set-wallpaper-help = Utiliser l’image comme fond d’écran du bureau.
palette-share = Partager…
palette-share-help = Envoyer le fichier à une autre app avec le menu de partage du système.
palette-send-by-email = Envoyer par e-mail…
palette-send-by-email-help = Créer un nouvel e-mail dans votre app de messagerie avec le fichier en pièce jointe.
# machine-drafted, needs native review
palette-share-with-zynk = Partager avec Zynk…
palette-share-with-zynk-help = Créer un lien web vers le fichier dans l’app Zynk.
palette-send-with-zynk = Envoyer avec Zynk…
palette-send-with-zynk-help = Envoyer le fichier à un contact ou à l’un de vos appareils avec Zynk.
palette-export-settings = Exporter les paramètres…
palette-export-settings-help = Enregistrer tous les paramètres et vos raccourcis clavier dans un fichier.
palette-import-settings = Importer les paramètres…
palette-import-settings-help = Lire les paramètres depuis un fichier, après avoir montré ce qui changerait.

disabled-in-archive-file = Cette page est dans une archive et n’a pas de fichier propre
disabled-not-rotatable = Ce type de fichier ne peut pas enregistrer de rotation
disabled-not-on-this-desktop = Votre bureau ne le propose pas

## Paramètres

settings-browsing-rotate-trim-edges = Rogner les bords JPEG pour pivoter sans perte
settings-browsing-rotate-trim-edges-help = Un JPEG dont la taille n’est pas un multiple de 8 ou 16 pixels ne peut pivoter sans perte qu’en coupant ses derniers pixels. Sinon, sa balise d’orientation est modifiée à la place, et le rognage vous est proposé à chaque fois.
settings-browsing-rotate-keep-modified = Conserver la date de modification lors d’une rotation
settings-browsing-rotate-keep-modified-help = Un fichier pivoté garde sa date de dernière modification et reste à sa place dans un tri par date.
settings-browsing-recent-folders = Dossiers récents
settings-browsing-recent-folders-help = Derniers dossiers utilisés pour copier et déplacer.

## Fenêtres

fileop-cancel = Annuler
rename-title = Renommer
rename-field-label = Nom
rename-button = Renommer
rename-sidecars-follow = Également renommés : { $files }
rename-extension-changed = Changer l’extension ne convertit pas le fichier ; d’autres apps pourraient ne pas l’ouvrir.
rename-problem-empty = Saisissez un nom.
rename-problem-invalid-char = Les noms ne peuvent pas contenir « { $char } ».
rename-problem-reserved = « { $name } » est un nom que Windows réserve aux périphériques.
rename-problem-too-long = Ce nom est trop long ; 255 octets au maximum.
rename-problem-dot = « . » et « .. » ne peuvent pas servir de noms.
rename-problem-trailing = Les noms ne peuvent pas finir par une espace ou un point.
rename-problem-exists = Un autre fichier porte déjà ce nom.
rename-problem-read-only = Ce dossier ne peut pas être modifié.
rename-problem-duplicate = Deux fichiers recevraient ce nom.

batch-rename-title =
    { $count ->
        [one] Renommer { $count } fichier
       *[other] Renommer { $count } fichiers
    }
batch-rename-field-label = Modèle
batch-rename-tokens = Utilisez { "{" }name{ "}" }, { "{" }n:3{ "}" } (001, 002, …), { "{" }date:%Y-%m-%d{ "}" }, { "{" }camera{ "}" }, { "{" }w{ "}" }×{ "{" }h{ "}" } et { "{" }ext{ "}" }.
batch-rename-summary =
    { $count ->
        [one] { $count } fichier sera renommé
       *[other] { $count } fichiers seront renommés
    }{ $problems ->
        [0] .
        [one] ; { $problems } ne peut pas l’être et reste inchangé.
       *[other] ; { $problems } ne peuvent pas l’être et restent inchangés.
    }
batch-rename-reading = Lecture des dates et des appareils…
batch-rename-preview = Aperçu
batch-rename-unchanged = garde son nom
batch-rename-more =
    { $count ->
        [one] { $count } de plus ci-dessous
       *[other] { $count } de plus ci-dessous
    }

transfer-copy-title-one = Copier « { $file } » vers un dossier
transfer-copy-title =
    { $count ->
        [one] Copier { $count } fichier vers un dossier
       *[other] Copier { $count } fichiers vers un dossier
    }
transfer-move-title-one = Déplacer « { $file } » vers un dossier
transfer-move-title =
    { $count ->
        [one] Déplacer { $count } fichier vers un dossier
       *[other] Déplacer { $count } fichiers vers un dossier
    }
transfer-recent = Dossiers récents
transfer-folder = Dossier
transfer-choose-folder = Choisir un dossier…
transfer-copy-dialog = Copier vers un dossier
transfer-move-dialog = Déplacer vers un dossier
transfer-sidecars =
    { $count ->
        [one] { $count } fichier annexe suit.
       *[other] { $count } fichiers annexes suivent.
    }
transfer-taken =
    { $count ->
        [one] { $count } nom est déjà utilisé dans { $folder }.
       *[other] { $count } noms sont déjà utilisés dans { $folder }.
    }
transfer-when-taken = Si un nom est déjà utilisé
transfer-skip = Ignorer
transfer-keep-both = Garder les deux
transfer-replace-option = Remplacer
transfer-copy = Copier
transfer-move = Déplacer
transfer-replace-title =
    { $count ->
        [one] Remplacer { $count } fichier dans { $folder } ?
       *[other] Remplacer { $count } fichiers dans { $folder } ?
    }
transfer-replace-note = Les fichiers présents vont à la corbeille. Annuler les fait revenir.
transfer-replace = Remplacer
transfer-back = Retour
transfer-stop = Arrêter

settings-file-import-title = Importer les paramètres
settings-file-import-dialog = Importer les paramètres
settings-file-export-dialog = Exporter les paramètres
settings-file-import-changes = Modifications
settings-file-import-change = { $before } → { $after }
settings-file-import-no-key = aucune touche
settings-file-import-unknown = Pas un paramètre de cette version ; ignoré.
settings-file-import-keymap = Raccourcis clavier
settings-file-import-nothing = Rien ne changerait : ces paramètres sont déjà appliqués.
settings-file-import-summary =
    { $count ->
        [one] { $count } modification. L’import peut être annulé dans les Paramètres.
       *[other] { $count } modifications. L’import peut être annulé dans les Paramètres.
    }
settings-file-import-secrets =
    { $count ->
        [one] { $count } entrée ressemble à un mot de passe ou une clé et n’est jamais importée.
       *[other] { $count } entrées ressemblent à des mots de passe ou des clés et ne sont jamais importées.
    }
settings-file-import-commands =
    { $count ->
        [one] { $count } commande utilisateur n’est pas importée : les commandes lancent des programmes. Copiez-la vous-même dans votre fichier de paramètres si vous lui faites confiance.
       *[other] { $count } commandes utilisateur ne sont pas importées : les commandes lancent des programmes. Copiez-les vous-même dans votre fichier de paramètres si vous leur faites confiance.
    }
settings-file-import-button = Importer

## Résultats

fileop-renamed = Renommé en { $file }
fileop-renamed-count =
    { $count ->
        [one] { $count } fichier renommé
       *[other] { $count } fichiers renommés
    }
fileop-copied =
    { $count ->
        [one] { $count } fichier copié dans { $folder }
       *[other] { $count } fichiers copiés dans { $folder }
    }
fileop-moved =
    { $count ->
        [one] { $count } fichier déplacé dans { $folder }
       *[other] { $count } fichiers déplacés dans { $folder }
    }
fileop-skipped =
    { $count ->
        [one] { $count } ignoré : son nom y est déjà utilisé.
       *[other] { $count } ignorés : leurs noms y sont déjà utilisés.
    }
fileop-stopped = Arrêté ; ce qui est fait reste fait.
fileop-failed-file =
    { $count ->
        [one] Impossible de copier ou déplacer { $file } : { $reason }
       *[other] Impossible de copier ou déplacer { $count } fichiers ; le premier, { $file } : { $reason }
    }
fileop-progress-copy =
    { $count ->
        [one] Copie de { $count } fichier… { PERCENT($percent) }
       *[other] Copie de { $count } fichiers… { PERCENT($percent) }
    }
fileop-progress-move =
    { $count ->
        [one] Déplacement de { $count } fichier… { PERCENT($percent) }
       *[other] Déplacement de { $count } fichiers… { PERCENT($percent) }
    }
fileop-done-status = Terminé
fileop-busy = La dernière modification de fichiers est en cours ; réessayez dans un instant.
fileop-nothing-to-redo = Rien à rétablir
fileop-undone = Annulé : { $what }
fileop-redone = Rétabli : { $what }
fileop-kind-rename = changement de nom
fileop-kind-batch-rename = changement de nom de plusieurs fichiers
fileop-kind-move = déplacement vers un dossier
fileop-kind-copy = copie vers un dossier
fileop-kind-rotate = rotation
rename-failed = Impossible de renommer : { $reason }

undo-problem-changed = Annulation impossible : { $file } a été modifié depuis, et annuler perdrait cette modification.
undo-problem-missing = Annulation impossible : { $file } n’existe plus.
undo-problem-taken = Annulation impossible : un autre fichier s’appelle maintenant { $file }.
undo-problem-io = Impossible d’annuler la modification de { $file } : { $reason }

rotate-done-lossless = { $file } pivoté sans aucune perte.
rotate-done-lossless-trimmed = { $file } pivoté sans aucune perte ; son bord a été rogné à { $width } × { $height }.
rotate-done-tag = { $file } pivoté via sa balise d’orientation.
rotate-done-sidecar = Rotation de { $file } enregistrée dans { $sidecar } ; le fichier RAW lui-même n’est jamais modifié.
rotate-tag-unreadable = Ses données d’image n’ont pas pu être lues assez loin pour le faire pivoter.
rotate-tag-multi-picture = Il contient plusieurs images, et faire pivoter la première abîmerait les autres.
rotate-tag-trailer = Des données suivent l’image, que la rotation déplacerait.
rotate-tag-not-aligned = Faire pivoter ses pixels sans perte le rognerait à { $width } × { $height }.
rotate-trim-offer = Rogner et pivoter
rotate-problem-unsupported = { $file } est un type de fichier qui ne peut pas enregistrer de rotation.
rotate-problem-too-large = { $file } est trop grand pour pivoter.
rotate-problem-damaged = { $file } est endommagé là où son orientation est stockée : { $reason }
rotate-problem-io = Impossible de faire pivoter { $file } : { $reason }

share-failed = Impossible de partager : { $reason }
email-failed = Impossible d’ouvrir votre app de messagerie : { $reason }
# machine-drafted, needs native review
zynk-opening = Ouverture de Zynk…
zynk-failed = Impossible de joindre Zynk : { $reason }
zynk-get-title-send = « Envoyer avec Zynk » nécessite l’app Zynk
zynk-get-title-share = « Partager avec Zynk » nécessite l’app Zynk
zynk-get-about = Zynk est une app gratuite pour Mac, Windows, Linux, iPhone et Android. Elle envoie photos, vidéos et dossiers entiers directement vers vos autres appareils et à d’autres personnes, quelle que soit leur taille. Les transferts sont chiffrés de bout en bout et reprennent là où ils s’étaient arrêtés si la connexion est coupée. Pour un destinataire qui n’a pas Zynk, elle crée un lien web depuis lequel télécharger les fichiers dans son navigateur.
zynk-get-macos = Téléchargez-la sur le Mac App Store. Une fois installée, { -app-name } lui transmet directement vos fichiers.
zynk-get-windows = Téléchargez-la sur le Microsoft Store. Une fois installée, { -app-name } lui transmet directement vos fichiers.
zynk-get-linux = Zynk pour Linux est une AppImage, sans rien à installer : téléchargez-la, placez-la dans ~/Applications et rendez-la exécutable (dans ses Propriétés, ou avec chmod +x). { -app-name } la trouve là, et n’importe où ailleurs tant que Zynk est ouvert.
zynk-get = Obtenir Zynk
zynk-send-old-windows = « Envoyer avec Zynk » nécessite Windows 10 version 2004 ou ultérieure. « Partager avec Zynk » fonctionne sur ce Windows.
settings-file-export-done = Paramètres enregistrés dans { $file }
settings-file-export-failed = Impossible d’enregistrer les paramètres : { $reason }
settings-file-import-unreadable = Impossible de lire { $file } : { $reason }
settings-file-import-done =
    { $count ->
        [one] { $count } modification importée
       *[other] { $count } modifications importées
    }
