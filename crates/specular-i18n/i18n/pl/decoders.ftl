# machine-drafted, needs native review
## Optional decoders: other vendors' free software that opens a few formats (Blackmagic RAW,
## DWG), used only when the user installed it. Specular never bundles or downloads it.
## $format is the format ("Blackmagic RAW", "DWG"), $decoder the decoder's own name
## ("Blackmagic RAW SDK", "ODA File Converter"), $vendor its maker ("Blackmagic Design",
## "Open Design Alliance"): names, left as they are.

## Settings › Optional decoders. The switches and paths are shown in the panel below
## "Look again", not as rows of their own.

settings-section-decoders = Opcjonalne dekodery
settings-decoders-look-again = Szukaj ponownie
settings-decoders-look-again-help = Szuka dekoderów zainstalowanych w międzyczasie.
    { -app-name } szuka też za każdym razem, gdy jego okno przechodzi na pierwszy plan.
settings-decoders-braw = Używaj Blackmagic RAW SDK
settings-decoders-braw-help = Klipy Blackmagic RAW otwierają się za jego pomocą, gdy jest zainstalowany.
settings-decoders-braw-path = Blackmagic RAW SDK
settings-decoders-braw-path-help = Zostaw puste, aby szukać tam, gdzie umieszcza go instalator.
settings-decoders-dwg = Używaj ODA File Converter
settings-decoders-dwg-help = Rysunki DWG otwierają się za jego pomocą, gdy jest zainstalowany.
    Kopia rysunku jest konwertowana w prywatnym folderze, który potem zostaje usunięty.
settings-decoders-dwg-path = Program ODA File Converter
settings-decoders-dwg-path-help = Zostaw puste, aby szukać tam, gdzie umieszcza go instalator.
settings-decoders-occt = Używaj OpenCASCADE
settings-decoders-occt-help = Modele STEP i IGES otwierają się za jego pomocą, gdy jest zainstalowany.
settings-decoders-occt-path = Biblioteki OpenCASCADE
settings-decoders-occt-path-help = Zostaw puste, aby szukać ich tam, gdzie system trzyma biblioteki.
# machine-drafted, needs native review
settings-decoders-czi = Używaj libCZI
settings-decoders-czi-help = Pliki Zeiss CZI otwierają się za jego pomocą, gdy jest zainstalowany.
    Działa w izolowanym procesie pomocniczym { -app-name }, bez dostępu do innych twoich plików.
settings-decoders-czi-path = Folder biblioteki libCZI
settings-decoders-czi-path-help = Zostaw puste, aby szukać tam, gdzie system przechowuje biblioteki.

## The panel: one block per decoder.

decoders-intro = Niektóre formaty otwierają się za pomocą darmowego oprogramowania ich twórców.
    Zainstaluj je raz: { -app-name } je znajdzie i użyje. Sam nigdy go nie pobiera.
decoders-keywords = opcjonalne dekodery producent oprogramowanie kodek wtyczka wymaga dekodera optional decoders vendor SDK codec plugin Blackmagic RAW BRAW RED R3D DWG ODA File Converter OpenCASCADE STEP IGES Zeiss CZI libCZI needs decoder
# A decoder's row: "Blackmagic RAW (.braw)". The tick box before it switches it on and off.
decoders-row = { $format } ({ $extensions })
# The row's value.
decoders-state-found = Znaleziono
decoders-state-off = Wyłączone
decoders-state-not-found = Nie zainstalowano
decoders-state-broken = Nie można użyć
decoders-state-refused = Niemożliwe
decoders-state-planned = Jeszcze niedostępne
decoders-state-checking = Szukanie…
# The row's lines.
decoders-uses = Otwiera się za pomocą: { $decoder } od { $vendor }.
decoders-not-found = Jest darmowy. Zainstaluj go, a potem naciśnij Szukaj ponownie.
decoders-off = Wyłączone: te pliki się nie otwierają.
decoders-version = Wersja { $version }.
decoders-r3d-why = Licencja RED pozwala czytać pliki R3D tylko programom zbudowanym z jej SDK na własnych warunkach RED.
    { -app-name } nie może przyjąć tych warunków, więc pliki R3D się nie otwierają.
decoders-ecw-why = Licencja firmy Hexagon pozwala używać ECW SDK tylko programom, które je dostarczają, a nie kopii zainstalowanej przez Ciebie.
    Dlatego pliki ECW się nie otwierają. Pliki JPEG 2000 z ECW SDK tak.
decoders-mrsid-why = Licencja MrSID SDK pozwala go używać tylko programom, które je dostarczają, a nie kopii zainstalowanej przez Ciebie.
    Dlatego pliki MrSID się nie otwierają.
# machine-drafted, needs native review
# DJI radiometric JPEGs (drone thermal cameras): the photo and metadata open; the
# temperatures would need DJI's SDK.
decoders-dji-thermal-why = Licencja SDK firmy DJI pozwala go używać tylko programom, które je dostarczają, a nie kopii zainstalowanej przez Ciebie. Zdjęcie i jego metadane i tak się otwierają.
decoders-occt-why = Bryły STEP i IGES wymagają OpenCASCADE, którego ta wersja programu { -app-name } nie może użyć.
# Rows with one button each, under a decoder.
decoders-get-row = Strona pobierania twórcy
decoders-get = Pobierz
decoders-locate-row = Zainstalowano gdzie indziej?
decoders-locate = Wskaż…
decoders-forget-row = Wybrano ręcznie.
decoders-forget = Zapomnij…
# Asked before Forget… does anything.
decoders-forget-ask = Zapomnieć ręcznie wybrany dekoder { $decoder }?
decoders-forget-detail = { -app-name } znów będzie go szukać sam. Nic nie zostanie usunięte z dysku.
decoders-forget-ok = Zapomnij
# The title of the Locate… file dialog.
decoders-locate-title = Wskaż dekoder { $decoder }
# Toasts.
decoders-locate-wrong = To nie jest { $decoder }.
decoders-located = Używany jest wybrany dekoder { $decoder }.
decoders-forgotten = { -app-name } znów sam szuka dekodera { $decoder }.
decoders-still-missing = Nadal nie znaleziono: { $decoder }.
decoders-now-found = Znaleziono: { $decoder }.
decoders-looked = Ponownie wyszukano dekodery.

## The card shown in place of a file that needs one. $file is the file's name.

decoders-card-title = { $format } wymaga darmowego dekodera od { $vendor }
decoders-card-what = { $file } otwiera się za pomocą: { $decoder }. Jest darmowy.
decoders-card-how = Zainstaluj go i wróć: { -app-name } sam go znajdzie.
decoders-card-get = Pobierz
decoders-card-installed = Już zainstalowane
decoders-card-locate = Wskaż…

## The gallery: the badge on such a file's thumbnail, and a line of its tooltip.

gallery-badge-needs-decoder = Wymaga dekodera
gallery-needs-decoder = Otwiera się za pomocą darmowego dekodera { $decoder } od { $vendor }, który nie jest zainstalowany.

## Why a file did not open, after "Can't open FILE:".

error-reason-needs-decoder = Pliki { $format } wymagają darmowego dekodera { $decoder } od { $vendor }, który nie jest zainstalowany.
