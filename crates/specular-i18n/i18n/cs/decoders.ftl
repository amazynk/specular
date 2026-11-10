# machine-drafted, needs native review
## Optional decoders: other vendors' free software that opens a few formats (Blackmagic RAW,
## DWG), used only when the user installed it. Specular never bundles or downloads it.
## $format is the format ("Blackmagic RAW", "DWG"), $decoder the decoder's own name
## ("Blackmagic RAW SDK", "ODA File Converter"), $vendor its maker ("Blackmagic Design",
## "Open Design Alliance"): names, left as they are.

## Settings › Optional decoders. The switches and paths are shown in the panel below
## "Look again", not as rows of their own.

settings-section-decoders = Volitelné dekodéry
settings-decoders-look-again = Hledat znovu
settings-decoders-look-again-help = Vyhledá dekodéry nainstalované mezitím.
    { -app-name } hledá také pokaždé, když se jeho okno dostane do popředí.
settings-decoders-braw = Používat Blackmagic RAW SDK
settings-decoders-braw-help = Klipy Blackmagic RAW se jím otevírají, když je nainstalovaný.
settings-decoders-braw-path = Blackmagic RAW SDK
settings-decoders-braw-path-help = Ponechte prázdné, aby se hledal tam, kam ho ukládá instalátor.
settings-decoders-dwg = Používat ODA File Converter
settings-decoders-dwg-help = Výkresy DWG se jím otevírají, když je nainstalovaný.
    Kopie výkresu se převede v soukromé složce, která se pak smaže.
settings-decoders-dwg-path = Program ODA File Converter
settings-decoders-dwg-path-help = Ponechte prázdné, aby se hledal tam, kam ho ukládá instalátor.
settings-decoders-occt = Používat OpenCASCADE
settings-decoders-occt-help = Modely STEP a IGES se jím otevírají, když je nainstalovaný.
settings-decoders-occt-path = Knihovny OpenCASCADE
settings-decoders-occt-path-help = Ponechte prázdné, aby se hledaly tam, kde systém uchovává knihovny.
# machine-drafted, needs native review
settings-decoders-czi = Používat libCZI
settings-decoders-czi-help = Soubory Zeiss CZI se jím otevírají, když je nainstalovaný.
    Běží v izolovaném pomocném procesu aplikace { -app-name } bez přístupu k vašim dalším souborům.
settings-decoders-czi-path = Složka knihovny libCZI
settings-decoders-czi-path-help = Ponechte prázdné, aby se hledala tam, kde systém ukládá knihovny.

## The panel: one block per decoder.

decoders-intro = Některé formáty se otevírají bezplatným softwarem od svých tvůrců.
    Stačí ho jednou nainstalovat: { -app-name } ho najde a použije. Sám ho nikdy nestahuje.
decoders-keywords = volitelné dekodéry výrobce software kodek zásuvný modul potřebuje dekodér optional decoders vendor SDK codec plugin Blackmagic RAW BRAW RED R3D DWG ODA File Converter OpenCASCADE STEP IGES Zeiss CZI libCZI needs decoder
# A decoder's row: "Blackmagic RAW (.braw)". The tick box before it switches it on and off.
decoders-row = { $format } ({ $extensions })
# The row's value.
decoders-state-found = Nalezeno
decoders-state-off = Vypnuto
decoders-state-not-found = Nenainstalováno
decoders-state-broken = Nelze použít
decoders-state-refused = Není možné
decoders-state-planned = Zatím není k dispozici
decoders-state-checking = Hledá se…
# The row's lines.
decoders-uses = Otevírá se pomocí: { $decoder } od { $vendor }.
decoders-not-found = Je zdarma. Nainstalujte ho a pak stiskněte Hledat znovu.
decoders-off = Vypnuto: tyto soubory se neotevřou.
decoders-version = Verze { $version }.
decoders-r3d-why = Licence RED dovoluje číst soubory R3D jen programům vytvořeným s jejím SDK za vlastních podmínek RED.
    { -app-name } tyto podmínky nemůže přijmout, proto se soubory R3D neotevřou.
decoders-ecw-why = Licence společnosti Hexagon dovoluje používat ECW SDK jen programům, které ho dodávají, ne kopii, kterou jste nainstalovali.
    Soubory ECW se proto neotevřou. Soubory JPEG 2000 z ECW SDK ano.
decoders-mrsid-why = Licence MrSID SDK dovoluje jeho použití jen programům, které ho dodávají, ne kopii, kterou jste nainstalovali.
    Soubory MrSID se proto neotevřou.
# machine-drafted, needs native review
# DJI radiometric JPEGs (drone thermal cameras): the photo and metadata open; the
# temperatures would need DJI's SDK.
decoders-dji-thermal-why = Licence SDK od DJI dovoluje jeho použití jen programům, které ho dodávají, ne kopii, kterou jste nainstalovali. Fotografie a její metadata se přesto otevřou.
decoders-occt-why = Tělesa STEP a IGES potřebují OpenCASCADE, které tato verze programu { -app-name } nemůže použít.
# Rows with one button each, under a decoder.
decoders-get-row = Stránka ke stažení od tvůrce
decoders-get = Získat
decoders-locate-row = Nainstalováno jinde?
decoders-locate = Vyhledat…
decoders-forget-row = Vybrali jste ho ručně.
decoders-forget = Zapomenout…
# Asked before Forget… does anything.
decoders-forget-ask = Zapomenout ručně vybraný dekodér { $decoder }?
decoders-forget-detail = { -app-name } ho bude znovu hledat sám. Nic na disku se neodstraní.
decoders-forget-ok = Zapomenout
# The title of the Locate… file dialog.
decoders-locate-title = Vyhledat dekodér { $decoder }
# Toasts.
decoders-locate-wrong = Toto není { $decoder }.
decoders-located = Používá se vybraný dekodér { $decoder }.
decoders-forgotten = { -app-name } znovu hledá dekodér { $decoder } sám.
decoders-still-missing = Dekodér { $decoder } stále nebyl nalezen.
decoders-now-found = Dekodér { $decoder } byl nalezen.
decoders-looked = Dekodéry byly znovu vyhledány.

## The card shown in place of a file that needs one. $file is the file's name.

decoders-card-title = { $format } potřebuje bezplatný dekodér od { $vendor }
decoders-card-what = { $file } se otevírá pomocí: { $decoder }. Je zdarma.
decoders-card-how = Nainstalujte ho a vraťte se: { -app-name } ho najde sám.
decoders-card-get = Získat
decoders-card-installed = Už je nainstalovaný
decoders-card-locate = Vyhledat…

## The gallery: the badge on such a file's thumbnail, and a line of its tooltip.

gallery-badge-needs-decoder = Potřebuje dekodér
gallery-needs-decoder = Otevírá se bezplatným dekodérem { $decoder } od { $vendor }, který není nainstalovaný.

## Why a file did not open, after "Can't open FILE:".

error-reason-needs-decoder = Soubory { $format } potřebují bezplatný dekodér { $decoder } od { $vendor }, který není nainstalovaný.
