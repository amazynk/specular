# machine-drafted, needs native review
# Specular — Rapport d’ouverture : pourquoi l’ouverture d’une image a pris ce temps (panneau
# d’informations, section « Performances » ; DESIGN.md §12.7).

open-report-title = Performances

## Headlines
open-report-fast = Ouverte en { $ms } ms
open-report-display-cache = Affichée depuis le cache d’affichage
open-report-slow-read = Stockage lent : { $percent } du temps a servi à lire le fichier ({ $speed } Mo/s)
# machine-drafted, needs native review
open-report-slow-read-no-speed = Stockage lent : { $percent } du temps a servi à lire le fichier (petit fichier : surtout l’attente de la réponse du stockage)
open-report-archive-read = { $percent } du temps a servi à décompresser l’archive
open-report-startup = { $percent } du temps a servi à démarrer { -app-name }
open-report-checked = Décodée deux fois : le premier résultat du décodeur matériel a été comparé à celui du CPU ({ $percent } du temps)
open-report-hardware = Décodée par le décodeur matériel, qui a pris { $percent } du temps
open-report-decode = Grande image : décoder { $mpx } mégapixels a pris { $percent } du temps
open-report-upload = { $percent } du temps a servi à envoyer l’image au GPU
open-report-waiting = { $percent } du temps a servi à attendre la fin d’autres tâches
open-report-stage = { $stage } a pris { $percent } du temps
open-report-memory-short = La mémoire manquait, le système a pu devoir utiliser le swap

## Why a decode ran on the processor although a hardware decoder can do it
open-report-sw-not-probed = Décodée par logiciel : le décodeur matériel était encore en cours de détection
open-report-sw-no-device = Décodée par logiciel : { $api } n’est pas disponible sur cet ordinateur
open-report-sw-no-codec = Décodée par logiciel : { $api } n’a pas de décodeur { $codec } sur ce GPU
open-report-sw-blocked-mismatch = Décodée par logiciel : le décodeur matériel a été désactivé car son résultat ne correspondait pas à celui du CPU
open-report-sw-blocked-error = Décodée par logiciel : le décodeur matériel a été désactivé après une erreur
open-report-sw-blocked-lost = Décodée par logiciel : le décodeur matériel a été désactivé car il ne répondait plus
open-report-sw-failed = Décodée par logiciel : le décodeur matériel a échoué sur cette image
open-report-sw-busy = Décodée par logiciel : le décodeur matériel était occupé
open-report-sw-unsupported = Décodée par logiciel : le décodeur matériel ne prend pas cette image en charge
open-report-sw-unavailable = Décodée par logiciel : le décodage matériel est désactivé jusqu’au redémarrage de { -app-name }
open-report-sw-cpu-only = Décodée par logiciel : le traitement est réglé sur « CPU uniquement »
open-report-sw-setting-off = Décodée par logiciel : le décodage matériel est désactivé dans les paramètres
open-report-sw-unchecked = Décodée par logiciel : le décodeur matériel n’est pas encore vérifié (il est d’abord essayé sur une image préparée en arrière-plan)
open-report-sw-lower-power = Décodée par logiciel : sur batterie, le CPU consomme moins pour cela
open-report-sw-faster = Décodée par logiciel : plus rapide sur cet ordinateur (environ { $cpu } ms, contre { $hw } ms avec le décodeur matériel)
open-report-sw-scaled = Décodée par logiciel : le décodeur matériel ne décode les images qu’en taille réelle

## Hints
open-report-hint-copy-local = Copiez le fichier sur un disque local pour l’ouvrir plus vite.
open-report-hint-extract = Extrayez l’archive dans un dossier pour ouvrir ses pages plus vite.
open-report-hint-try-again = Pour réessayer le décodeur matériel : Paramètres → Performances → Réessayer.
open-report-hint-automatic = Réglez « Où les images sont traitées » sur Automatique dans Paramètres → Performances.
open-report-hint-turn-on = Réglez « Décodage matériel » sur Automatique dans Paramètres → Performances.
open-report-hint-free-memory = Fermez d’autres programmes pour libérer de la mémoire.
open-report-hint-once-only = Seule la première image est vérifiée ainsi.

## Rows
open-report-total = Jusqu’à la netteté
open-report-first-pixels = Premiers pixels
# machine-drafted, needs native review
open-report-whole = Image entière affichée
open-report-stage-startup = Démarrage
open-report-stage-read = Lecture du fichier
open-report-stage-parse = Lecture de l’en-tête
open-report-stage-colour = Préparation de la gestion des couleurs
open-report-stage-decode = Décodage
open-report-stage-upload = Envoi au GPU
open-report-stage-waiting = Attente d’autres tâches
open-report-decoder = Décodée par
open-report-decoder-checked = { $engine }, vérifié par le CPU
open-report-decoder-cache = Cache d’affichage
# $scale is the reduction: 4 for a level a quarter of the full width and height.
open-report-decoder-scaled = { $engine }, à 1/{ $scale } de la taille
open-report-decoder-scaled-full = { $engine }, en taille réelle, puis réduite à 1/{ $scale }
open-report-decoder-scaled-no-index = { $engine }, à 1/{ $scale } de la taille, sans l’index de démarrage rapide
open-report-colour-conversion = Couleurs converties par

## Advisor: a pattern across recent opens, what would help and by how much (rows)
# $count and $total are counts of opens, $format a file format ("HEIC"), $speed a formatted
# number (MB/s), $low and $high formatted numbers ("2.5").
open-report-advice-recurring = Récurrent
open-report-advice-change = Ce qui aiderait
open-report-advice-gain = Gain estimé
open-report-advice-software = { $count } des { $total } dernières images { $format } lentes { $count ->
    [one] a été décodée
   *[other] ont été décodées
} par logiciel pour cette raison
open-report-advice-slow-storage = { $count } des { $total } dernières images lentes { $count ->
    [one] a été ralentie
   *[other] ont été ralenties
} par la lecture du fichier (environ { $speed } Mo/s)
# machine-drafted, needs native review
open-report-advice-slow-storage-no-speed = { $count } des { $total } dernières images lentes { $count ->
    [one] a été ralentie
   *[other] ont été ralenties
} par la lecture du fichier (petits fichiers : surtout l’attente de la réponse du stockage)
open-report-advice-archive = { $count } des { $total } dernières images lentes { $count ->
    [one] a été ralentie
   *[other] ont été ralenties
} par la décompression de l’archive
open-report-advice-hardware-decoder = Un GPU doté d’un décodeur matériel { $codec }
open-report-advice-gain-range = Environ { $low } à { $high } fois plus rapide
open-report-advice-gain-one = Environ { $low } fois plus rapide

## Performance insights: the indicator in the corner and the panel it opens
# machine-drafted, needs native review
# $total is a formatted duration ("240 ms"), $format a file format ("HEIC").
open-report-insights-title = Conseils de performances
open-report-insights-indicator = Pourquoi est-ce lent ?
open-report-insights-announce = Conseil de performances : des images continuent de s’ouvrir lentement pour la même raison. Ouvrez « Pourquoi est-ce lent ? » pour voir ce qui aiderait.
open-report-insights-hide = Masquer
open-report-insights-last = Dernière image lente : { $total } jusqu’à la netteté
open-report-insights-now = Ce que vous pouvez faire maintenant
open-report-insights-hardware = Matériel qui aiderait
open-report-insights-none = Aucun ralentissement récurrent
open-report-insights-none-detail = Les conseils apparaissent quand plusieurs images s’ouvrent lentement pour la même raison. Pour les trouver, { -app-name } conserve une semaine d’ouvertures lentes sur cet ordinateur ; rien n’est envoyé nulle part.
open-report-insights-software = Les images { $format } sont décodées par le CPU
open-report-insights-storage = La lecture des fichiers retarde les images
open-report-insights-archive = La décompression des archives retarde les images
open-report-insights-nothing-now = Aucun paramètre de { -app-name } n’y change rien.
open-report-insights-display-cache = Activez le cache d’affichage : les images déjà ouvertes s’ouvrent alors depuis une copie rapide sur cet ordinateur.
open-report-insights-prefetch = Gardez davantage d’images voisines prêtes, pour que les suivantes soient préparées pendant que vous regardez.
open-report-insights-hw-ssd = Un disque interne rapide (un SSD NVMe) pour ces fichiers
open-report-insights-action-turn-on = Activer
open-report-insights-action-try-again = Réessayer
open-report-insights-action-automatic = Régler sur Automatique
open-report-insights-dismiss-storage = Ne plus afficher pour le stockage
open-report-insights-dismiss-decoder = Ne plus afficher pour le décodage
open-report-insights-off = Désactiver les conseils
open-report-insights-on = Activer les conseils
open-report-insights-restore = Réafficher les conseils masqués
open-report-insights-power = Cet ordinateur économise l’énergie (sur batterie ou en mode économie d’énergie), ce qui ralentit le décodage par le CPU. Sur secteur, les images peuvent s’ouvrir plus vite.

## Performance insights in the gallery: the same, about making thumbnails
# machine-drafted, needs native review
# $count and $total are counts of thumbnails, $total in -last-thumbnail a formatted duration
# ("240 ms"), $format a file format ("HEIC"), $speed a formatted number (MB/s).
open-report-insights-announce-thumbnails = Conseil de performances : des vignettes continuent d’apparaître lentement pour la même raison. Ouvrez « Pourquoi est-ce lent ? » pour voir ce qui aiderait.
open-report-insights-last-thumbnail = Dernière vignette lente : { $total } pour la créer
open-report-insights-thumbnails-software = Les vignettes des images { $format } sont décodées par le CPU
open-report-insights-thumbnails-storage = La lecture des fichiers retarde les vignettes
open-report-insights-thumbnails-archive = La décompression des archives retarde les vignettes
open-report-insights-thumbnail-cache = Les vignettes ne peuvent pas être conservées sur cet ordinateur (le cache des vignettes est indisponible) ; elles sont donc recréées à chaque affichage du dossier.
open-report-advice-thumbnails-software = { $count } des { $total } dernières vignettes { $format } lentes { $count ->
    [one] a été décodée
   *[other] ont été décodées
} par logiciel pour cette raison
open-report-advice-thumbnails-slow-storage = { $count } des { $total } dernières vignettes lentes { $count ->
    [one] a été ralentie
   *[other] ont été ralenties
} par la lecture du fichier (environ { $speed } Mo/s)
# machine-drafted, needs native review
open-report-advice-thumbnails-slow-storage-no-speed = { $count } des { $total } dernières vignettes lentes { $count ->
    [one] a été ralentie
   *[other] ont été ralenties
} par la lecture du fichier (petits fichiers : surtout l’attente de la réponse du stockage)
open-report-advice-thumbnails-archive = { $count } des { $total } dernières vignettes lentes { $count ->
    [one] a été ralentie
   *[other] ont été ralenties
} par la décompression de l’archive
