# machine-drafted, needs native review
# Specular — Importation depuis les appareils photo, téléphones et cartes mémoire. Français.
# La carte est seulement lue ; les fichiers n’y sont supprimés que sur demande, après une copie vérifiée.

palette-import = Importer des photos…
disabled-no-import-source = Aucune carte mémoire, aucun appareil photo ni téléphone connecté

## Paramètres → Importation
settings-section-import = Importation
settings-import-on-card = À l’insertion d’une carte
settings-import-on-card-help = Ce qui se passe quand une carte mémoire, un appareil photo ou un téléphone contenant des photos est connecté.
settings-import-on-card-ask = Demander
settings-import-on-card-open = Ouvrir l’importation
settings-import-on-card-nothing = Ne rien faire
settings-import-destination = Importer dans
settings-import-destination-help = Le dossier où vont les photos et vidéos importées. S’il n’est pas défini, elles vont dans votre dossier Images.
settings-import-destination-unset = Dossier Images
settings-import-folder-pattern = Dossiers
settings-import-folder-pattern-help = Les dossiers créés à l’intérieur : {"{"}year{"}"}, {"{"}month{"}"}, {"{"}day{"}"}, {"{"}camera{"}"} et {"{"}event{"}"}, séparés par /.
settings-import-rename-pattern = Renommer
settings-import-rename-pattern-help = Nouveaux noms de fichier, comme dans Convertir : {"{"}name{"}"}, {"{"}n:03{"}"}, {"{"}date:%Y%m%d{"}"}, {"{"}camera{"}"}, {"{"}event{"}"}. Vide, les noms de l’appareil sont gardés.
settings-import-keep-pairs = Garder RAW et JPEG ensemble
settings-import-keep-pairs-help = Un fichier RAW et le JPEG pris avec lui vont dans un même dossier sous un même nom, même renommés.
settings-import-low-res-videos = Importer les copies vidéo basse résolution
settings-import-low-res-videos-help = Certains appareils enregistrent une petite copie d’aperçu de chaque vidéo (.LRV). Désactivé, elles restent sur la carte.
settings-import-eject = Éjecter à la fin
settings-import-eject-help = Éjecte la carte une fois tout copié, pour pouvoir la retirer.

## La proposition à l’insertion d’une carte
import-offer = Importer depuis { $name } ?
import-offer-button = Importer

## La vue d’importation
import-title = Importer
import-from = depuis { $name }
import-source-next = Source suivante
import-close = Fermer
import-scanning = Recherche de nouvelles photos… { $done } sur { $total }
import-scanning-start = Recherche de nouvelles photos…
import-none-new = Tout le contenu de { $name } a déjà été importé.
import-empty = Il n’y a aucune photo ni vidéo sur { $name }.
import-no-source = Aucune carte mémoire, aucun appareil photo ni téléphone connecté.
import-summary = { $count ->
    [one] { $count } nouvel élément
    [many] { $count } nouveaux éléments
   *[other] { $count } nouveaux éléments
} · { $size }
import-already = { $count } déjà importés
import-unreadable = { $count ->
    [one] { $count } fichier illisible
    [many] { $count } fichiers illisibles
   *[other] { $count } fichiers illisibles
}
import-selected = { $count } sur { $total } sélectionnés
import-select-all = Tout sélectionner
import-select-none = Tout désélectionner
import-badge-pair = RAW+JPEG
import-badge-raw = RAW
import-badge-video = Vidéo

import-destination = Importer dans
import-choose = Choisir…
import-folder-pattern = Dossiers
import-event = Événement
import-event-hint = Facultatif, pour {"{"}event{"}"}
import-rename = Renommer
import-rename-hint = Vide, les noms de l’appareil sont gardés
import-preview = Premier fichier : { $path }
import-pattern-error = { $error }
import-keep-pairs = Garder RAW et JPEG ensemble
import-eject = Éjecter à la fin
import-delete = Supprimer de la carte après l’importation
import-free = { $size } à copier · { $free } d’espace libre
import-free-unknown = { $size } à copier
import-no-space = Pas assez d’espace libre dans { $folder } : cette importation nécessite { $need } et seulement { $free } est libre. Libérez de l’espace ou choisissez un autre dossier.
import-start = { $count ->
    [one] Importer { $count } élément
    [many] Importer { $count } éléments
   *[other] Importer { $count } éléments
}
import-nothing-selected = Sélectionnez les photos à importer

import-confirm-delete = { $count ->
    [one] Une fois copié et vérifié, { $count } fichier sera supprimé de { $name }. Cette action est irréversible.
    [many] Une fois copiés et vérifiés, { $count } fichiers seront supprimés de { $name }. Cette action est irréversible.
   *[other] Une fois copiés et vérifiés, { $count } fichiers seront supprimés de { $name }. Cette action est irréversible.
}
import-confirm-yes = Importer et supprimer
import-confirm-no = Revenir

import-progress = Importation de { $done } sur { $total } · { $size }
import-progress-eta = Importation de { $done } sur { $total } · { $size } · { $time }
import-paused = En pause · { $done } sur { $total }
import-pause = Pause
import-resume = Reprendre
import-cancel = Annuler
import-hide = Masquer
import-stopping = Arrêt…
import-running = Une importation est déjà en cours.

import-done = { $count ->
    [one] { $count } fichier importé
    [many] { $count } fichiers importés
   *[other] { $count } fichiers importés
} dans { $folder }
import-done-existed = { $count ->
        [one] { $count } s’y trouvait déjà
       *[other] { $count } s’y trouvaient déjà
    }
import-done-deleted = { $count } supprimés de la carte
import-done-failed = { $count ->
    [one] { $count } fichier n’a pas pu être importé
    [many] { $count } fichiers n’ont pas pu être importés
   *[other] { $count } fichiers n’ont pas pu être importés
}
import-done-cancelled = Importation annulée après { $count ->
        [one] { $count } fichier
       *[other] { $count } fichiers
    }. Une nouvelle importation reprend là où elle s’est arrêtée.
import-done-nothing = Rien de nouveau n’a été importé.
import-ejected = { $name } peut être retiré.
import-eject-failed = Impossible d’éjecter { $name } : { $error }
import-failed-destination = Le dossier { $folder } ne peut pas être utilisé : { $error }
import-ledger-failed = La liste des fichiers importés n’a pas pu être lue ({ $error }) ; des fichiers déjà importés peuvent être proposés à nouveau.

## Lecteur d’écran
import-a11y-group = Importation
import-a11y-tile = { $name }, { $date }
import-a11y-tile-selected = { $name }, { $date }, sélectionné
