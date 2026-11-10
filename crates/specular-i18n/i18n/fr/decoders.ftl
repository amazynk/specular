# machine-drafted, needs native review

settings-section-decoders = Décodeurs facultatifs
settings-decoders-look-again = Chercher à nouveau
settings-decoders-look-again-help = Chercher les décodeurs installés depuis.
    { -app-name } cherche aussi chaque fois que sa fenêtre passe au premier plan.
settings-decoders-braw = Utiliser le Blackmagic RAW SDK
settings-decoders-braw-help = Les clips Blackmagic RAW s’ouvrent avec lui quand il est installé.
settings-decoders-braw-path = Blackmagic RAW SDK
settings-decoders-braw-path-help = Laisser vide pour le trouver là où son installateur le met.
settings-decoders-dwg = Utiliser ODA File Converter
settings-decoders-dwg-help = Les dessins DWG s’ouvrent avec lui quand il est installé.
    Une copie du dessin est convertie dans un dossier privé, qui est ensuite supprimé.
settings-decoders-dwg-path = Programme ODA File Converter
settings-decoders-dwg-path-help = Laisser vide pour le trouver là où son installateur le met.
settings-decoders-occt = Utiliser OpenCASCADE
settings-decoders-occt-help = Les modèles STEP et IGES s’ouvrent avec lui quand il est installé.
settings-decoders-occt-path = Bibliothèques OpenCASCADE
settings-decoders-occt-path-help = Laisser vide pour les trouver là où le système range les bibliothèques.
# machine-drafted, needs native review
settings-decoders-czi = Utiliser libCZI
settings-decoders-czi-help = Les fichiers Zeiss CZI s’ouvrent avec lui quand il est installé.
    Il s’exécute dans l’assistant isolé de { -app-name }, sans accès à vos autres fichiers.
settings-decoders-czi-path = Dossier de la bibliothèque libCZI
settings-decoders-czi-path-help = Laisser vide pour la trouver là où le système range les bibliothèques.

decoders-intro = Certains formats s’ouvrent avec un logiciel gratuit de leur éditeur.
    Installez-le une fois : { -app-name } le trouve et l’utilise. Il ne le télécharge jamais.
decoders-keywords = décodeurs facultatifs éditeur SDK codec module Blackmagic RAW BRAW RED R3D DWG ODA File Converter OpenCASCADE STEP IGES Zeiss CZI libCZI décodeur requis
decoders-row = { $format } ({ $extensions })
decoders-state-found = Trouvé
decoders-state-off = Désactivé
decoders-state-not-found = Non installé
decoders-state-broken = Inutilisable
decoders-state-refused = Impossible
decoders-state-planned = Pas encore disponible
decoders-state-checking = Recherche…
decoders-uses = S’ouvre avec { $decoder } de { $vendor }.
decoders-not-found = C’est gratuit. Installez-le, puis cliquez sur Chercher à nouveau.
decoders-off = Désactivé : ces fichiers ne s’ouvrent pas.
decoders-version = Version { $version }.
decoders-r3d-why = La licence de RED ne permet de lire les fichiers R3D qu’aux programmes construits avec son SDK, selon les conditions de RED.
    { -app-name } ne peut pas accepter ces conditions, donc les fichiers R3D ne s’ouvrent pas.
decoders-ecw-why = La licence de Hexagon ne permet d’utiliser son ECW SDK qu’aux programmes qui le fournissent, pas à une copie que vous avez installée.
    Les fichiers ECW ne s’ouvrent donc pas. Les fichiers JPEG 2000 créés par l’ECW SDK, si.
decoders-mrsid-why = La licence du MrSID SDK ne permet de l’utiliser qu’aux programmes qui le fournissent, pas à une copie que vous avez installée.
    Les fichiers MrSID ne s’ouvrent donc pas.
# machine-drafted, needs native review
# DJI radiometric JPEGs (drone thermal cameras): the photo and metadata open; the
# temperatures would need DJI's SDK.
decoders-dji-thermal-why = La licence du SDK de DJI ne permet de l’utiliser qu’aux programmes qui le fournissent, pas à une copie que vous avez installée. La photo et ses métadonnées s’ouvrent quand même.
decoders-occt-why = Les solides STEP et IGES nécessitent OpenCASCADE, que cette version de { -app-name } ne peut pas utiliser.
decoders-get-row = La page de téléchargement de l’éditeur
decoders-get = Télécharger
decoders-locate-row = Installé ailleurs ?
decoders-locate = Choisir…
decoders-forget-row = Choisi à la main.
decoders-forget = Oublier…
decoders-forget-ask = Oublier votre choix pour { $decoder } ?
decoders-forget-detail = { -app-name } le cherche de nouveau lui-même. Rien n’est supprimé du disque.
decoders-forget-ok = Oublier
decoders-locate-title = Choisir { $decoder }
decoders-locate-wrong = Ce n’est pas { $decoder }.
decoders-located = Votre choix pour { $decoder } est utilisé.
decoders-forgotten = { -app-name } cherche de nouveau { $decoder } lui-même.
decoders-still-missing = { $decoder } est toujours introuvable.
decoders-now-found = { $decoder } trouvé.
decoders-looked = Décodeurs recherchés à nouveau.

decoders-card-title = { $format } nécessite le décodeur gratuit de { $vendor }
decoders-card-what = { $file } s’ouvre avec { $decoder }. C’est gratuit.
decoders-card-how = Installez-le, puis revenez : { -app-name } le trouve tout seul.
decoders-card-get = Télécharger
decoders-card-installed = Je l’ai installé
decoders-card-locate = Choisir…

gallery-badge-needs-decoder = Décodeur requis
gallery-needs-decoder = S’ouvre avec { $decoder } de { $vendor }, gratuit mais non installé.

error-reason-needs-decoder = Les fichiers { $format } nécessitent { $decoder } de { $vendor }, gratuit mais non installé.
