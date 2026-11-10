# machine-drafted, needs native review
# Local copies for offline editing (the Library).

settings-library-local-copies-limit = Limite des copies locales
settings-library-local-copies-limit-help = La place maximale que les copies locales peuvent prendre dans le dossier de données de Specular.

local-keep-menu = Garder une copie locale…
local-remove-menu = Retirer la copie locale…
local-keep-location-menu = Garder une copie locale de cet emplacement…
local-remove-location-menu = Retirer la copie locale de cet emplacement…
local-keep-album-menu = Garder une copie locale de cet album…
local-remove-album-menu = Retirer la copie locale de cet album…
local-remove-held-elsewhere = Ces copies appartiennent à un emplacement ou un album gardé. Retirez-les là-bas.

local-keep-ask = Garder une copie locale de { $item } ?
local-keep-detail = { $count ->
    [one] 1 photo.
    [many] { $count } photos.
   *[other] { $count } photos.
}
    Originaux : les fichiers tels quels.
    Proxys : les raws à 2 560 pixels, assez pour le développement. Les autres fichiers sont copiés en entier.
    Les copies vont dans le dossier de données de Specular. Les originaux ne sont pas modifiés.
local-keep-originals = Garder les originaux
local-keep-proxies = Garder les proxys
local-keeping = Copie de { $item }…
local-kept = { $item } : { $count ->
    [one] 1 copie locale
    [many] { $count } copies locales
   *[other] { $count } copies locales
}, { $size }.
local-kept-full = { $count ->
    [one] 1 photo ne tenait pas dans la limite de taille.
    [many] { $count } photos ne tenaient pas dans la limite de taille.
   *[other] { $count } photos ne tenaient pas dans la limite de taille.
}
local-kept-unreachable = { $count ->
    [one] 1 photo n’a pas pu être lue.
    [many] { $count } photos n’ont pas pu être lues.
   *[other] { $count } photos n’ont pas pu être lues.
}

local-remove-ask = Retirer la copie locale de { $item } ?
local-remove-detail = Les copies du dossier de données de Specular sont supprimées. Les originaux ne sont pas touchés. Les changements en attente sont gardés.
local-remove-ok = Retirer la copie locale
local-removed = Copie locale de { $item } retirée, { $size } libérés.

local-banner-offline = { $location } est hors ligne. Vous travaillez sur des copies locales.
local-banner-offline-many = { $count } emplacements sont hors ligne. Vous travaillez sur des copies locales.
local-banner-waiting = { $count ->
    [one] 1 changement attend l’original.
    [many] { $count } changements attendent les originaux.
   *[other] { $count } changements attendent les originaux.
}
local-banner-conflicts = { $count ->
    [one] 1 fichier a changé pendant que vous étiez hors ligne.
    [many] { $count } fichiers ont changé pendant que vous étiez hors ligne.
   *[other] { $count } fichiers ont changé pendant que vous étiez hors ligne.
}
local-review = Examiner…
local-sidebar-originals = copie locale
local-sidebar-proxies = proxys locaux

local-synced = { $count ->
    [one] 1 changement écrit dans son original.
    [many] { $count } changements écrits dans leurs originaux.
   *[other] { $count } changements écrits dans leurs originaux.
}
local-sync-failed = { $count ->
    [one] 1 changement n’a pas pu être écrit. Il reste en attente.
    [many] { $count } changements n’ont pas pu être écrits. Ils restent en attente.
   *[other] { $count } changements n’ont pas pu être écrits. Ils restent en attente.
}

local-conflict-title = { $file } a changé pendant que vous étiez hors ligne. Quelle version garder : { $what } ?
local-conflict-yours = La vôtre : { $text }
local-conflict-theirs = Sur le disque : { $text }
local-conflict-later = Annuler décide plus tard. Rien n’est écrasé.
local-keep-mine = Garder la vôtre
local-keep-theirs = Garder celle du fichier
local-what-marks = note
local-what-keywords = mots-clés
local-what-fields = champs IPTC
local-what-edit = retouche
local-stars = { $count ->
    [0] Aucune étoile
    [one] 1 étoile
    [many] { $count } étoiles
   *[other] { $count } étoiles
}
local-label = { $label ->
    [1] Rouge
    [2] Jaune
    [3] Vert
   *[4] Bleu
}
local-pick = Retenue
local-reject = Rejetée
local-no-keywords = aucun mot-clé
local-changed-elsewhere = changé ailleurs
local-photo-changed = la photo elle-même a changé
local-edit-here = retouchée ici, { $when }
local-edit-elsewhere = retouchée ailleurs, { $when }
local-edit-none = aucune retouche

local-export-ask = { $count ->
    [one] 1 photo est hors ligne. L’exporter depuis son proxy ?
    [many] { $count } photos sont hors ligne. Les exporter depuis leurs proxys ?
   *[other] { $count } photos sont hors ligne. Les exporter depuis leurs proxys ?
}
local-export-detail = Les proxys font 2 560 pixels sur le grand côté. Les originaux donnent la taille complète.
local-export-proxy = Exporter depuis les proxys
local-export-wait = Attendre les originaux
local-export-waiting = L’export commence quand les originaux sont de retour.

local-panel-title = Copies locales
local-panel-used = { $used } utilisés sur { $limit }.
local-panel-originals = { $count ->
    [one] 1 original
    [many] { $count } originaux
   *[other] { $count } originaux
}
local-panel-proxies = { $count ->
    [one] 1 photo, raws en proxys
    [many] { $count } photos, raws en proxys
   *[other] { $count } photos, raws en proxys
}
local-panel-remove = Retirer…
local-panel-footer = Stockées dans { $folder }. Les originaux ne sont jamais modifiés.
local-panel-keywords = hors ligne copies locales proxys espace
