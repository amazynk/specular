# machine-drafted, needs native review

## Send to PACS (C-STORE, STOW-RS), DIMSE servers, teaching quiz and teaching case export.

## Servers: DIMSE fields and Verify.

dicomweb-field-kind = Type
dicomweb-kind-web = DICOMweb
dicomweb-kind-dimse = DICOM (DIMSE)
dicomweb-field-host = Hôte
dicomweb-hint-host = pacs.example.org
dicomweb-field-port = Port
dicomweb-hint-port = 104 ou 11112
# The application entity title the server answers to.
dicomweb-field-ae = AE title du serveur
dicomweb-hint-ae = PACS
# The application entity title Specular sends as.
dicomweb-field-our-ae = Notre AE title
# machine-drafted, needs native review
dicomweb-field-tls = Connexion sécurisée (TLS)
dicomweb-tls-on = Connexion sécurisée activée
dicomweb-tls-off = Connexion sécurisée désactivée
dicomweb-problem-host = Saisissez le nom d’hôte ou l’adresse du serveur.
dicomweb-problem-port = Le port est un nombre de 1 à 65535.
dicomweb-problem-ae = Un AE title compte de 1 à 16 caractères, sans barre oblique inverse.
# machine-drafted, needs native review
dicomweb-dimse-note-open = Les serveurs DICOM (DIMSE) permettent la recherche, l’ouverture des séries et l’envoi.
# How series are fetched: C-GET (over our own connection) or C-MOVE (the server connects back).
# machine-drafted, needs native review
pacs-field-retrieve = Récupérer avec
# machine-drafted, needs native review
pacs-retrieve-get = C-GET
# machine-drafted, needs native review
pacs-retrieve-move = C-MOVE
# C-MOVE: the address of this computer and the port the server sends to.
# machine-drafted, needs native review
pacs-field-receive-address = Adresse de réception
# machine-drafted, needs native review
pacs-hint-receive-address = Adresse de cet ordinateur, par ex. 192.168.1.20
# machine-drafted, needs native review
pacs-field-receive-port = Port de réception
# machine-drafted, needs native review
pacs-hint-receive-port = Par ex. 11113
# machine-drafted, needs native review
pacs-move-note = Pour C-MOVE, le serveur doit connaître notre AE title { $ae } avec cette adresse et ce port.
    Specular n’y écoute que pendant l’arrivée d’une série.
# machine-drafted, needs native review
pacs-problem-receive-address = Saisissez l’adresse de cet ordinateur (par ex. 192.168.1.20) sur laquelle recevoir.
# machine-drafted, needs native review
pacs-problem-receive-port = Saisissez un port de réception. C-MOVE en a besoin.
dicomweb-verify = Vérifier
dicomweb-verifying = Vérification du serveur…
dicomweb-verify-ok = { $server } a répondu.

## The Studies browser and the palette.

studies-send = Envoyer au PACS…
studies-send-description = Envoie cette série, son étude ou tout le contenu du disque à un serveur.
palette-send-to-pacs = Envoyer au PACS…
palette-send-to-pacs-help = Envoyer la série ou l’étude en cours à un serveur PACS.

## The Send page.

pacs-title = Envoyer au PACS
# Under the title: what is being sent.
pacs-summary = { $studies ->
    [one] { $studies } étude
    [many] { $studies } études
   *[other] { $studies } études
} · { $files ->
    [one] { $files } fichier
    [many] { $files } fichiers
   *[other] { $files } fichiers
}
pacs-what = Envoyer
pacs-scope-series = Cette série ({ $files ->
    [one] { $files } fichier
    [many] { $files } fichiers
   *[other] { $files } fichiers
})
pacs-scope-study = Cette étude ({ $files ->
    [one] { $files } fichier
    [many] { $files } fichiers
   *[other] { $files } fichiers
})
pacs-scope-all = Tout ce qui a été lu ({ $studies ->
    [one] { $studies } étude
    [many] { $studies } études
   *[other] { $studies } études
}, { $files ->
    [one] { $files } fichier
    [many] { $files } fichiers
   *[other] { $files } fichiers
})
pacs-anonymise = Anonymiser
pacs-anonymise-note = Les données du patient sont retirées des copies envoyées. Vos fichiers ne sont pas modifiés.
pacs-identified-note = Les fichiers sont envoyés tels quels, avec les données du patient.
# The list of servers to choose from.
pacs-to = Vers
pacs-no-servers = Aucun serveur pour l’instant. Ajoutez-en un dans Serveurs DICOMweb.
pacs-send = Envoyer…
pacs-go-back = Retour
pacs-confirm-send = Envoyer
pacs-confirm = Envoyer { $studies ->
    [one] { $studies } étude
    [many] { $studies } études
   *[other] { $studies } études
} ({ $files ->
    [one] { $files } fichier
    [many] { $files } fichiers
   *[other] { $files } fichiers
}) à { $server } ?
pacs-busy = Un envoi est encore en cours.
pacs-started = Envoi en cours.
pacs-stopping = Arrêt…
pacs-sending = Envoi de { $done } sur { $total } à { $server }…
pacs-sent-all = { $total ->
    [one] { $total } fichier a été envoyé à { $server }.
    [many] Les { $total } fichiers ont été envoyés à { $server }.
   *[other] Les { $total } fichiers ont été envoyés à { $server }.
}
pacs-sent-some = { $sent } fichiers sur { $total } ont été envoyés à { $server }.
pacs-stopped = Arrêté après { $sent } sur { $total ->
        [one] { $total } fichier
       *[other] { $total } fichiers
    }.
pacs-stopped-because = Arrêté après { $sent } sur { $total ->
        [one] { $total } fichier
       *[other] { $total } fichiers
    } : { $reason }
pacs-warnings = { $count ->
    [one] { $count } fichier a été stocké avec un avertissement.
    [many] { $count } fichiers ont été stockés avec un avertissement.
   *[other] { $count } fichiers ont été stockés avec un avertissement.
}
# A file that was not sent: its file name and why.
pacs-failed-line = { $name } : { $reason }
pacs-failed-more = Et { $count } de plus.

## Why one file was not sent.

pacs-reason-unreadable = Lecture impossible : { $reason }
pacs-reason-not-dicom = Pas un fichier DICOM
pacs-reason-burned-in = Des données du patient sont incrustées dans l’image, il n’est donc pas envoyé anonymisé
pacs-reason-document = Un document qui ne peut pas être anonymisé
pacs-reason-anonymise = Anonymisation impossible : { $reason }
pacs-reason-not-accepted = Le serveur n’accepte pas { $class }
pacs-reason-convert = Conversion pour le serveur impossible : { $reason }
pacs-reason-resources = Le serveur manque d’espace ou de ressources
pacs-reason-mismatch = Selon le serveur, les données ne correspondent pas à leur type
pacs-reason-understand = Le serveur n’a pas pu le lire
pacs-reason-class = Le serveur ne prend pas en charge ce type d’objet
pacs-reason-not-authorised = Le serveur ne l’autorise pas
pacs-reason-status = Le serveur l’a refusé (statut { $code })
pacs-reason-refused = Le serveur l’a refusé
pacs-reason-too-large = Trop volumineux pour le serveur
pacs-reason-http = Le serveur l’a refusé (erreur { $status })

## Why sending stopped, or Verify failed.

pacs-fatal-network-off = Le réseau est désactivé.
pacs-fatal-unknown-host = { $host } n’est pas un nom d’hôte connu.
pacs-fatal-connect = Rien ne répond sur { $host }, port { $port }.
pacs-fatal-unreachable = { $host } est injoignable.
pacs-fatal-timeout = { $host } n’a pas répondu à temps.
pacs-fatal-cancelled = Arrêté.
pacs-fatal-called-ae = { $host } ne répond pas à l’AE title { $ae }. Vérifiez l’AE title du serveur.
pacs-fatal-calling-ae = { $host } ne connaît pas notre AE title { $ae }. Ajoutez-le sur le serveur.
pacs-fatal-busy = { $host } est occupé. Réessayez plus tard.
pacs-fatal-rejected = { $host } a refusé la connexion.
pacs-fatal-aborted = { $host } a mis fin à la connexion.
pacs-fatal-nothing-accepted = { $host } n’accepte aucun de ces types d’objets.
pacs-fatal-protocol = { $host } a répondu d’une façon que Specular ne comprend pas.
pacs-fatal-auth = { $host } n’a pas accepté la connexion avec ces identifiants.
pacs-fatal-http = { $host } a répondu avec l’erreur { $status }.

## Teaching quiz.

palette-teaching-quiz = Quiz d’enseignement
palette-teaching-quiz-help = Parcourir les images clés ou un dossier d’enseignement, les observations masquées jusqu’à ce que vous les révéliez.
# "Case 2 of 5".
quiz-case = Cas { $n } sur { $total }
quiz-hint-hidden = Espace révèle les observations · → cas suivant · Échap termine
quiz-hint-revealed = Espace les masque à nouveau · → cas suivant · Échap termine
quiz-hidden = Observations masquées.
quiz-no-findings = Aucune observation enregistrée.
quiz-notes = Notes de cas : { $notes }
quiz-ended = Quiz terminé.
quiz-empty = Aucun cas à afficher. Marquez des images clés avec K ou choisissez un dossier d’enseignement.
quiz-not-a-case = Ce dossier n’est pas un dossier d’enseignement exporté par Specular.
quiz-pick-title = Choisir un dossier d’enseignement

## Teaching case folder (Export key images).

figure-keys-case = Dossier d’enseignement
figure-case-notes = Notes de cas
figure-case-findings = Observations
figure-case-none = Aucune enregistrée
figure-case-readme = Dossier d’enseignement exporté par Specular. Les données du patient sont retirées.
    images : les images clés, texte incrusté masqué en noir.
    DICOM : des copies anonymisées et un manifeste Key Object Selection « For Teaching ».
    case.txt : les notes de cas et les observations de chaque image. case.json : la même chose pour les scripts.
    Pour Radiopaedia : téléversez le dossier DICOM ou les images, et copiez les notes depuis case.txt.

## Search on a DIMSE server (C-FIND).
# machine-drafted, needs native review
pacs-find-not-supported = { $host } ne répond pas aux recherches (C-FIND).
pacs-find-status = { $host } a refusé la recherche (statut { $code }).

## Opening a series from a DIMSE server (C-GET, C-MOVE).
# machine-drafted, needs native review
pacs-retrieve-no-get = { $host } n’envoie pas de séries par C-GET.
    Choisissez C-MOVE dans les paramètres du serveur.
# machine-drafted, needs native review
pacs-retrieve-no-move = { $host } n’accepte pas les requêtes C-MOVE.
# machine-drafted, needs native review
pacs-retrieve-unknown-destination = { $host } ne connaît pas notre AE title { $ae }.
    Ajoutez-le sur le serveur avec l’adresse de cet ordinateur et le port { $port }.
# machine-drafted, needs native review
pacs-retrieve-resources = { $host } manque de ressources. Réessayez plus tard.
# machine-drafted, needs native review
pacs-retrieve-status = { $host } a refusé d’envoyer la série (statut { $code }).
# machine-drafted, needs native review
pacs-retrieve-no-matches = { $host } n’a aucune image pour cette série.
# machine-drafted, needs native review
pacs-retrieve-roles = { $host } a refusé d’envoyer des images par cette connexion.
    Choisissez C-MOVE dans les paramètres du serveur.
# machine-drafted, needs native review
pacs-retrieve-move-nothing = { $host } n’a pas pu envoyer les images à cet ordinateur.
    Vérifiez l’adresse, le port et le pare-feu, et que le serveur connaît notre AE title { $ae }.
# machine-drafted, needs native review
pacs-retrieve-nothing = { $host } n’a pu envoyer { $count ->
    [one] l’image
    [many] aucune des { $count } images
   *[other] aucune des { $count } images
} sous une forme lisible par Specular.
# machine-drafted, needs native review
pacs-retrieve-listen = Impossible de recevoir sur { $address }, port { $port } : { $reason }

## DICOM over TLS.
# machine-drafted, needs native review
pacs-fatal-tls-certificate = Le certificat de { $host } n’est pas approuvé.
    Son émetteur doit figurer parmi les certificats approuvés du système.
pacs-fatal-tls = Aucune connexion sécurisée à { $host }.
    Vérifiez que le port { $port } utilise DICOM sur TLS.
