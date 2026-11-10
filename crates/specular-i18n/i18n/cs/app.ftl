# machine-drafted, needs native review
# Specular — rozhraní prohlížeče, paleta příkazů, chybová hlášení, okno O aplikaci. Čeština.
#
# Konvence (úplný seznam viz crates/specular-i18n/src/lib.rs)
# - Čísla se předávají jako čísla ($count, $index, $value…) a formátuje je ICU4X podle
#   aktuální oblasti; pište { $count }, ne předem naformátovaný text. Výběr množného čísla
#   používá stejnou proměnnou (kategorie one, few, many, other).
# - $file, $path, $shortcut, $reason apod. se předávají jako text a automaticky se obalí
#   izolačními znaky Unicode (FSI…PDI).
# - Klávesové zkratky přicházejí již lokalizované pro platformu ($shortcut = "Ctrl+O" / "⌘O").

-app-name = Specular

## Popisky a názvy pro usnadnění přístupu na panelu nástrojů

toolbar-open = Otevřít obrázek…
toolbar-prev = Předchozí obrázek
toolbar-next = Další obrázek
toolbar-back-gallery = Zpět do galerie
toolbar-zoom-out = Oddálit
toolbar-zoom-in = Přiblížit
toolbar-zoom-level = Úroveň přiblížení a předvolby
toolbar-zoom-fit = Přizpůsobit oknu
toolbar-zoom-100 = Skutečná velikost ({ PERCENT(100) })
toolbar-rotate-left = Otočit doleva
toolbar-rotate-right = Otočit doprava
toolbar-fullscreen-enter = Celá obrazovka
toolbar-fullscreen-exit = Ukončit celou obrazovku
toolbar-info = Informace o obrázku
toolbar-settings = Nastavení
toolbar-delete = Přesunout do koše
toolbar-copy = Kopírovat obrázek
toolbar-palette = Paleta příkazů
toolbar-go-to = Přejít na obrázek…
toolbar-go-to-range = 1–{ $count }
# machine-drafted, needs native review
toolbar-customise = Přizpůsobit panel nástrojů…
toolbar-more = Další
toolbar-announce-moved = { $item }: pozice { $index } z { $count }
toolbar-announce-added = Přidáno { $item }: pozice { $index } z { $count }
toolbar-announce-removed = Odebráno z panelu nástrojů: { $item }
toolbar-announce-reset = Panel nástrojů obnoven na výchozí
# Popisek následovaný klávesovou zkratkou.
tooltip-with-shortcut = { $label } ({ $shortcut })
# Popisek ovládacího prvku, který teď nelze použít; $reason je jedna ze zpráv disabled-*.
tooltip-unavailable = { $label } (nedostupné: { $reason })
# Proč je ovládací prvek nebo příkaz nedostupný (popisky, paleta příkazů, čtečky obrazovky).
disabled-no-image = Není otevřen žádný obrázek
disabled-still-loading = Obrázek se ještě načítá
disabled-decode-failed = Tento obrázek nelze dekódovat
disabled-no-other-images = Ve složce nejsou další obrázky

## Údaje

# Pozice ve složce, např. „12 / 348“.
readout-position = { $index } / { $count }
# Čtečky obrazovky vysloví pro údaj o pozici.
readout-position-a11y = Obrázek { $index } z { $count }
# $zoom je již naformátované procento, např. „150 %“.
readout-zoom = { $zoom }
readout-zoom-a11y = Přiblížení { $zoom }
readout-zoom-fit = Přizpůsobeno ({ $zoom })
zoom-preset-fit = Přizpůsobit oknu
zoom-preset-fill = Vyplnit okno
zoom-preset-actual = Skutečná velikost
# machine-drafted, needs native review
# The last entry of the zoom presets: the window fills the screen (ticked while it does).
zoom-preset-fullscreen = Celá obrazovka
# $zoom je již naformátované procento.
zoom-preset-percent = { $zoom }
# Rozměry obrázku v pixelech, např. „6000 × 4000“.
readout-dimensions = { $width } × { $height }
# Počet obrázků v aktuální složce.
status-image-count =
    { $count ->
        [one] { $count } obrázek
        [few] { $count } obrázky
        [many] { $count } obrázku
       *[other] { $count } obrázků
    }

## Paleta příkazů

palette-placeholder = Zadejte příkaz nebo nastavení…
palette-no-results = Žádné odpovídající příkazy
# Změna nastavení z palety, např. „Nastavit: Kolečko myši → Další obrázek“.
palette-set-setting = Nastavit: { $setting } → { $value }
palette-next = Další obrázek
palette-prev = Předchozí obrázek
palette-first = První obrázek
palette-last = Poslední obrázek
palette-go-to = Přejít na obrázek…
palette-zoom-in = Přiblížit
palette-zoom-out = Oddálit
palette-zoom-fit = Přizpůsobit oknu
palette-zoom-fill = Vyplnit okno
# machine-drafted, needs native review
palette-zoom-fit-width = Přizpůsobit šířce
palette-zoom-fit-height = Přizpůsobit výšce
palette-zoom-100 = Přiblížit na { PERCENT(100) }
palette-zoom-200 = Přiblížit na { PERCENT(200) }
palette-zoom-300 = Přiblížit na { PERCENT(300) }
palette-zoom-400 = Přiblížit na { PERCENT(400) }
palette-zoom-rectangle = Přiblížit výběr
palette-rotate-left = Otočit doleva
palette-rotate-right = Otočit doprava
palette-flip-horizontal = Převrátit vodorovně
palette-flip-vertical = Převrátit svisle
palette-toggle-fullscreen = Přepnout celou obrazovku
palette-navigator = Zobrazit nebo skrýt navigátor
palette-loupe = Lupa
palette-pixel-grid = Zobrazit nebo skrýt mřížku pixelů
palette-info-panel = Zobrazit nebo skrýt panel informací
palette-copy-image = Kopírovat obrázek
# machine-drafted, needs native review
palette-read-codes = Přečíst kódy
# machine-drafted, needs native review
palette-find-text = Najít text
# machine-drafted, needs native review
palette-face-boxes = Zobrazit tváře
palette-copy-path = Kopírovat cestu k souboru
palette-move-to-trash = Přesunout do koše
palette-undo-trash = Vrátit přesun do koše
palette-open = Otevřít obrázek…
# machine-drafted, needs native review
palette-open-address = Otevřít adresu…
palette-settings = Nastavení
# machine-drafted, needs native review
palette-customise-toolbar = Přizpůsobit panel nástrojů…
palette-cheat-sheet = Klávesové zkratky
palette-hud = Zobrazit nebo skrýt HUD časů snímků
palette-language = Jazyk
palette-theme = Motiv
# machine-drafted, needs native review
palette-interface-larger = Větší rozhraní
palette-interface-smaller = Menší rozhraní
palette-interface-reset = Rozhraní na { PERCENT(100) }
palette-quit = Ukončit { -app-name }

## Přehled zkratek

cheatsheet-title = Klávesové zkratky
cheatsheet-navigation = Procházení
cheatsheet-zoom-pan = Přiblížení a posun
cheatsheet-view = Zobrazení
cheatsheet-file = Soubor
cheatsheet-window = Okno
cheatsheet-help = Nápověda
cheatsheet-hold = Podržet { $key }
cheatsheet-close-hint = Zavřete klávesou { $key } nebo Esc

## Chyby

# machine-drafted, needs native review
viewer-no-preview = Soubor neobsahuje náhled.
error-cannot-open = Nelze otevřít { $file }
error-cannot-open-reason = Nelze otevřít { $file }: { $reason }
error-unsupported-format = Soubor { $file } je ve formátu, který { -app-name } zatím neumí přečíst.
error-file-truncated = Soubor { $file } je neúplný. Zobrazuje se část, kterou se podařilo přečíst.
error-file-too-large = Soubor { $file } je příliš velký ({ $megapixels } megapixelů; limit je { $limit } megapixelů).
error-not-found = Soubor { $file } již neexistuje.
error-decoder-stopped = dekodér skončil, aniž by vytvořil obrázek
# machine-drafted, needs native review
# Shown after "Can’t open FILE:" for brain data (GIFTI .func/.shape/.label, CIFTI) that is
# drawn on a surface: no .surf.gii with that many vertices was found in the same folder.
error-needs-surface = potřebuje povrch mozku (soubor .surf.gii s { $vertices } vrcholy) ve stejné složce
error-permission-denied = Chybí oprávnění ke čtení souboru { $file }.
error-details = Podrobnosti
crash-last-time = { -app-name } se minule neočekávaně ukončil; zpráva byla uložena do { $path }
error-trash-failed = Soubor { $file } nelze přesunout do koše: { $reason }
# machine-drafted, needs native review
# Důvody, zobrazené za „Nelze otevřít SOUBOR:“ a dalšími zprávami „Nelze …:“,
# proto začínají malým písmenem.
error-reason-not-found = už tam není. Mohl být přesunut, přejmenován nebo smazán.
error-reason-permission = nemáte oprávnění jej číst. Zkontrolujte jeho vlastníka a oprávnění.
error-reason-disk-full = disk je plný. Uvolněte trochu místa a zkuste to znovu.
error-reason-read-only = disk je jen pro čtení.
error-reason-truncated = soubor je neúplný. Možná se stále kopíruje nebo stahuje.
error-reason-is-folder = je to složka, ne soubor.
# A remote store (a Zarr image on a server or in an S3 bucket).
error-reason-remote-denied = server odmítl přístup. Data mohou být soukromá; u bucketu S3 nejprve nastavte přihlašovací údaje AWS.
error-reason-remote-missing = na této adrese nejsou žádná data Zarr.
# $status is an HTTP status code ("500").
error-reason-remote-status = server odpověděl chybou { $status }.
error-reason-offline = server není dostupný. Zkontrolujte adresu a připojení.
# machine-drafted, needs native review
error-reason-network-off = síť je vypnutá v Nastavení › Soukromí.
error-reason-timed-out = jednotka nebo server neodpověděly včas. Zkontrolujte připojení a zkuste to znovu.
error-reason-memory = není dost paměti. Zavřete jiné obrázky nebo aplikace a zkuste to znovu.
error-reason-unsupported = je ve formátu, který { -app-name } zatím neumí přečíst.
# $detail jsou vlastní slova dekodéru, technická a anglicky („12-bit lossless JPEG“).
error-reason-unsupported-detail = používá funkci, kterou { -app-name } zatím neumí přečíst ({ $detail }).
# machine-drafted, needs native review
# A file whose extension several formats share (".img") and whose content is none of them.
# $formats lists the format names, e.g. "ERDAS IMAGINE, ADSC SMV, Analyze 7.5".
error-reason-not-one-of = není žádným ze souborů .{ $ext }, které { -app-name } čte ({ $formats }).
error-reason-damaged = soubor je poškozený nebo není tím, co uvádí jeho název ({ $detail }).
error-reason-too-large = je příliš velký na otevření.
# machine-drafted, needs native review
error-reason-needs-memory = potřebuje { $needed } volné paměti RAM, ale teď je volných jen { $free }. Zavřete některé jiné aplikace a zkuste to znovu, nebo jej otevřete na počítači s větší pamětí.
error-needs-memory = Tento obrázek potřebuje { $needed } volné paměti RAM, ale teď je volných jen { $free }. Zavřete některé jiné aplikace a zkuste to znovu, nebo jej otevřete na počítači s větší pamětí.
error-needs-memory-short = Potřebuje { $needed } volné RAM
error-reason-cancelled = otevírání bylo zastaveno.
error-reason-trash-access = už tam není, nebo nemáte oprávnění jej měnit.
error-reason-restore-collision = v jeho složce už je soubor se stejným názvem.
error-reason-trash-root = celou jednotku nelze přesunout do koše.
error-reason-not-in-trash = už není v koši.
error-reason-program-missing = program není nainstalovaný nebo byl přesunut. Zvolte jej znovu nebo jej přeinstalujte.
error-reason-program-not-allowed = program nemá povoleno se spustit. Zkontrolujte, zda je spustitelný.
# machine-drafted, needs native review
error-reason-permission-write = nemáte oprávnění jej měnit. Zkontrolujte jeho vlastníka a oprávnění.
error-reason-file-has-errors = obsahuje chyby. Nejprve je opravte, aby se z něj nic neztratilo.
error-reason-not-dicom = není to soubor DICOM.
error-reason-no-image = neobsahuje žádný obrázek.
error-reason-needs-layout = je to objem bez hlavičky. K jeho otevření je potřeba znát velikost, typ vzorků a pořadí bajtů.
error-reason-not-archive = není to archiv, který { -app-name } umí přečíst.
error-reason-needs-sandbox = tento druh archivu otevírá pouze chráněná čtečka aplikace { -app-name }, která není k dispozici.
error-reason-encrypted = je zašifrovaný a vyžaduje heslo.
error-reason-wrong-password = heslo je nesprávné.
error-reason-checksum = soubor je poškozený: jeho obsah neodpovídá kontrolnímu součtu.
error-reason-missing-entry = chybí v něm soubor.
error-reason-busy = používá jej jiný program. Zavřete jej tam a zkuste to znovu.
error-reason-catalogue-damaged = soubor katalogu je poškozený.
error-reason-cannot-open = jeho soubor nelze otevřít. Zkontrolujte, že jeho složka existuje a že do ní můžete zapisovat.
error-reason-newer-version = zapsala jej novější verze aplikace { -app-name }. Chcete-li jej změnit, aktualizujte { -app-name }.
error-reason-internal = uvnitř aplikace { -app-name } se něco pokazilo. Podrobnosti jsou v protokolu.
error-reason-volume-too-few = { $count ->
    [one] jeden řez je příliš málo na sestavení objemu.
    [few] { $count } řezy jsou příliš málo na sestavení objemu.
    [many] { $count } řezu je příliš málo na sestavení objemu.
   *[other] { $count } řezů je příliš málo na sestavení objemu.
}
error-reason-volume-no-geometry = řez neuvádí, kde leží ani kterým směrem je natočen.
error-reason-volume-not-parallel = řezy nejsou rovnoběžné.
error-reason-volume-shared-position = několik obrázků sdílí každou z pozic (fáze, echa nebo časová řada); pozic: { $count }.
error-reason-volume-missing = chybí řezy, přibližně: { $count }.
error-reason-volume-mismatch = řezy se liší velikostí nebo roztečí pixelů.
error-reason-volume-not-grey = řezy jsou barevné, nikoli v odstínech šedi.
error-reason-volume-skewed = osy objemu nesvírají pravý úhel.
error-reason-volume-too-large = potřebuje { $need } MiB a povoleno je jen { $budget } MiB.
error-reason-url-not-specular = není to odkaz aplikace { -app-name }.
error-reason-url-too-long = je příliš dlouhý.
error-reason-url-damaged = je poškozený. Mohl být při kopírování zkrácen nebo změněn.
error-reason-url-unknown = žádá o něco, co tato verze aplikace { -app-name } nezná. Může pomoci aktualizace { -app-name }.
error-reason-url-not-absolute = cesty k souborům musí být úplné.
error-reason-url-no-paths = neuvádí žádný soubor.
error-reason-url-too-many = uvádí příliš mnoho souborů.

## Oznámení

# machine-drafted, needs native review
# A background task (a listing, an export, a file operation) stopped part way; $reason is
# error-reason-internal.
background-failed = Nepodařilo se dokončit: { $reason }
toast-moved-to-trash = Přesunuto do koše
toast-moved-to-trash-named = Soubor { $file } byl přesunut do koše
toast-moved-to-trash-count =
    { $count ->
        [one] Do koše přesunut { $count } obrázek
        [few] Do koše přesunuty { $count } obrázky
        [many] Do koše přesunuto { $count } obrázku
       *[other] Do koše přesunuto { $count } obrázků
    }
toast-undo = Zpět
toast-restored = Soubor { $file } byl obnoven
toast-copied-image = Obrázek zkopírován
toast-copied-path = Cesta zkopírována
toast-copied-report = Zpráva zkopírována
toast-copied-value = Hodnota zkopírována
toast-loop-start = Zpět na první obrázek
toast-loop-end = Skok na poslední obrázek
# machine-drafted, needs native review
toast-interface-size = Velikost rozhraní { PERCENT($percent) }
toast-language-changed = Jazyk změněn na { $language }

## Prázdné stavy

empty-drop = Přetáhněte sem obrázek nebo stiskněte { $shortcut }
empty-open-button = Otevřít obrázek…
empty-drop-release = Uvolněním otevřete
empty-folder = V této složce nejsou žádné obrázky, které { -app-name } umí otevřít.
# machine-drafted, needs native review
# Pod „Otevřít obrázek…“ v prázdném okně: kde najít vše ostatní.
empty-hint-keys = Stiskněte { $cheatsheet } pro klávesové zkratky nebo { $palette } pro vyhledání libovolného příkazu.
empty-hint-cheatsheet = Stiskněte { $cheatsheet } pro klávesové zkratky.
empty-hint-palette = Stiskněte { $palette } pro vyhledání libovolného příkazu.
# Pod indikátorem načítání, když otevírání souboru trvá déle než chvilku.
viewer-opening = Otevírání { $file }…
fullscreen-exit-hint = Celou obrazovku ukončíte klávesou Esc
# Zobrazí se při přechodu na celou obrazovku; Esc nejprve zruší přiblížení.
fullscreen-back-hint = Esc obnoví přiblížení nebo ukončí celou obrazovku
# Přehled zkratek a paleta příkazů: co dělá Esc, jeden krok na stisk.
action-back = Zavřít panel (v dialogu nejprve o krok zpět), obnovit přiblížení, ukončit celou obrazovku; v galerii o složku výš

## Výběr jazyka

language-picker-title = Jazyk
# machine-drafted, needs native review
language-picker-search = Hledat jazyk
# $percent je již naformátované procento, např. „87 %“.
language-completeness = Přeloženo { $percent }
language-system-default = Podle systému ({ $language })
language-pseudo-accented = Pseudojazyk: s diakritikou a delší
language-pseudo-rtl = Pseudojazyk: zrcadlený zprava doleva

## Okno O aplikaci

about-title = O aplikaci { -app-name }
about-version = Verze { $version }
about-tagline = Rychlý a věrný prohlížeč obrázků.
about-copyright = © { $year } Autoři aplikace { -app-name }
about-license = Licencováno pod licencí MIT License / Apache License 2.0.
about-geonames = Názvy míst z GeoNames (geonames.org), licencováno pod Creative Commons Attribution 4.0.
about-unicode = Údaje o národním prostředí z Unicode CLDR prostřednictvím ICU4X. © Unicode, Inc.
# machine-drafted, needs native review
about-lensfun = Profily objektivů z databáze Lensfun (lensfun.github.io), licencováno pod Creative Commons Attribution-ShareAlike 3.0.
about-third-party = Licence třetích stran
about-third-party-file = { -app-name } obsahuje software a data třetích stran pod jejich vlastními licencemi; viz THIRD-PARTY.md, instalovaný spolu s aplikací { -app-name }.
about-copy-info = Kopírovat informace o systému
about-close = Zavřít

## Rozhraní prohlížeče (aplikace, fáze 1)

app-close = Zavřít
# Odznak na obrázku, jehož soubor předčasně končí.
app-truncated-badge = Neúplný soubor
app-info-summary = Souhrn
app-info-all-fields = Všechna pole
app-info-copy-value = Kopírovat hodnotu
app-info-copy-field = Kopírovat hodnotu pole { $field }
app-info-loading = Čtení metadat…
# machine-drafted, needs native review
# Všechna pole, pro soubor, který kromě pixelů neukládá žádná metadata.
app-info-no-fields = Tento soubor nemá žádná pole metadat.
# Gain-map row of the info summary (machine-drafted, needs native review)
app-info-gain-map = HDR mapa zisku
app-info-gain-map-headroom = Až { $headroom }× jas SDR
app-copy-failed = Nelze kopírovat: { $reason }
app-restore-failed = Soubor { $file } nelze obnovit: { $reason }
app-open-dialog-title = Otevřít obrázek
app-open-dialog-filter = Podporované soubory
app-open-dialog-all = Všechny soubory
app-open-group-images = Obrázky
app-open-group-raw = RAW z fotoaparátu
app-open-group-video = Video
app-open-group-sound = Zvuk
app-open-group-documents = Dokumenty
app-open-group-drawings = Výkresy a CAD
app-open-group-models = 3D modely
app-open-group-maps = Mapy a trasy
app-open-group-science = Věda a medicína
app-open-group-fonts = Písma
app-open-group-other = Ostatní

## Informace o vykreslování (O aplikaci, Nastavení › Pokročilé)

# machine-drafted, needs native review
renderer-title = Informace o vykreslování
renderer-section-adapter = Grafický adaptér
renderer-section-limits = Limity a funkce
renderer-section-display = Displej
renderer-section-memory = Paměť
renderer-section-decoders = Volitelné dekodéry
renderer-unknown = Neznámé
# The PCI vendor and device IDs ("1002:747e").
renderer-ids = ID výrobce a zařízení
# Wayland, X11, Windows or macOS.
renderer-display-server = Zobrazovací server
renderer-features = Použité funkce
renderer-surface-format = Formát okna
renderer-surface-formats = Nabízené formáty
renderer-present-mode = Režim zobrazení
renderer-present-modes = Nabízené režimy
renderer-hdr = HDR
# $headroom is how much brighter than white the display goes ("4.0").
renderer-hdr-on = Zapnuto (rezerva { $headroom }×)
renderer-hdr-off = Vypnuto (displej umí zobrazit HDR)
renderer-hdr-unavailable = Na tomto displeji není k dispozici
renderer-decode-budget = Dekódování smí použít
renderer-decode-budget-detail = { PERCENT(80) } volné paměti, sdílené všemi otevíranými obrázky.
renderer-display-cache = Mezipaměť zobrazení

## HUD výkonu (F12)

app-hud-scale = Měřítko
app-hud-level = Úroveň
app-hud-level-thumbnail = Vložená miniatura
app-hud-level-full = Plné rozlišení
# $denominator je 2, 4 nebo 8: obrázek dekódovaný v 1/2, 1/4 nebo 1/8 velikosti.
app-hud-level-scaled = 1/{ $denominator }
app-hud-open = Otevření
app-hud-decode = Dekódování
app-hud-upload = Nahrání
app-hud-frame = Snímek
app-hud-threads = Vlákna
app-hud-threads-value = { $threads } ({ $queued } ve frontě)
app-hud-tiles = Dlaždice
app-hud-tiles-value = { $tiles } z { $capacity }
app-hud-ram = Paměť
app-hud-vram = Paměť GPU
app-hud-gpu = GPU

# Potvrzení přesunu do koše
app-confirm-trash-title = Přesunout { $file } do koše?
# machine-drafted, needs native review
app-confirm-trash-sidecars = Doprovodné soubory půjdou také do koše: { $files }
app-cancel = Zrušit
# Varování před dekódováním velmi velkého obrázku. $need a $available jsou velikosti
# paměti („3,2 GiB“).
toast-low-memory = Tento obrázek může potřebovat asi { $need } paměti, ale k dispozici je jen { $available }. Může se načítat pomalu nebo selhat.

# Zpráva o hardwaru (Nastavení → Pokročilé). Hodnoty doplní aplikace.
hw-report-title = Zpráva o hardwaru
hw-report-copy = Kopírovat zprávu
hw-report-version = Verze
hw-report-cpu = CPU
hw-report-cores = Jádra
hw-report-cores-value = logická: { $logical }, fyzická: { $physical }
hw-report-performance-cores = Výkonná jádra
hw-report-cpu-features = Rychlé cesty CPU
hw-report-none = Žádné
hw-report-memory = Paměť
hw-report-memory-value = celkem { $total }, volné { $available }
hw-report-graphics-api = Grafické API
hw-report-driver = Ovladač
hw-report-max-texture = Největší textura
hw-report-pixels = { $width } × { $height } px
hw-report-decode-threads = Vlákna dekódování
hw-report-ram-cache = Mezipaměť obrázků v RAM
hw-report-vram-cache = Mezipaměť v paměti GPU
hw-report-processing = Zpracování
hw-report-path-jpeg-decode = Dekódování JPEG
hw-report-path-ycbcr-to-rgb = Převod barev
hw-report-path-png-unfilter = Dekódování PNG
hw-report-path-heic-decode = Dekódování HEIC
hw-report-path-inference = Analýza obrázků
hw-report-engine-cpu = CPU
hw-report-engine-cpu-parallel = CPU, paralelně
hw-report-engine-gpu = GPU
hw-report-engine-hw-decoder = hardwarový dekodér
hw-report-engine-npu = NPU
hw-report-build-debug = ladicí sestavení
hw-report-build-release = vydané sestavení

## Usnadnění přístupu (čtečky obrazovky)
# Obrázek, jak ho ohlásí čtečka obrazovky: název souboru a velikost v pixelech.
a11y-image = { $name }, { $width } × { $height } pixelů
# Oblast, kde se zobrazují krátká oznámení.
a11y-notifications = Oznámení
# machine-drafted, needs native review
a11y-transport-playback = Přehrávání
a11y-transport-pages = Stránky
a11y-settings-sections = Sekce

## Read codes (QR codes in the picture) (machine-drafted, needs native review)
codes-placeholder = Kódy v tomto obrázku
codes-looking = Hledání kódů…
codes-none = V tomto obrázku nebyly nalezeny žádné kódy.
codes-failed = Obrázek nelze přečíst: { $reason }
codes-copy = Kopírovat „{ $text }“
# $kind is "QR", $version a number.
codes-kind = { $kind } { $version }
# Data Matrix: its size in modules, e.g. "Data Matrix 12×12".
# machine-drafted, needs native review
codes-kind-size = { $kind } { $rows }×{ $cols }
codes-open-row = Otevřít odkaz na { $host }…
codes-network-off = Žádná síť není zapnutá, takže se odkazy neotevřou.
codes-open-title = Otevřít odkaz na { $host }?
codes-open-detail = Otevře se ve webovém prohlížeči. Otevírejte jen odkazy, kterým důvěřujete.
codes-open = Otevřít v prohlížeči
# machine-drafted, needs native review
# Find text: the text in the picture shown, read by the Local AI pack's Text models.
ocr-placeholder = Text v tomto obrázku
# machine-drafted, needs native review
ocr-looking = Čtení textu…
# machine-drafted, needs native review
ocr-none = V tomto obrázku nebyl nalezen žádný text.
# machine-drafted, needs native review
ocr-failed = Text nelze přečíst: { $reason }
# machine-drafted, needs native review
ocr-needs-pack = Funkce Najít text potřebuje textové modely balíčku místní AI (Nastavení › Místní AI).
# machine-drafted, needs native review
ocr-loading = Načítání textových modelů…
# machine-drafted, needs native review
# A row: choosing it copies the line. $text is the line (one line, shortened).
ocr-copy = Kopírovat „{ $text }“
# machine-drafted, needs native review
ocr-copy-all = Kopírovat veškerý text
# machine-drafted, needs native review
# A row's note: how sure the reading is, and the add-on that read it. $percent is formatted.
ocr-confidence = Jistota { $percent }

## Hardware report panel (machine-drafted, needs native review)
hw-report-section-system = Systém
hw-report-section-gpu = GPU
hw-report-section-decoding = Dekódování
hw-report-operating-system = Operační systém
hw-report-adapter = Adaptér
hw-report-gpu-type = Typ
hw-report-gpu-init = Doba spuštění
hw-report-max-layers = Vrstvy textur
hw-report-compression = Komprese textur
hw-report-unified-memory = Paměť sdílená s CPU
hw-report-display = Displej
hw-report-hw-decode = Hardwarové dekódování
hw-report-core-cluster = Skupina jader
hw-report-power = Napájení
hw-report-calibration = Měření rychlosti
hw-report-codec-others = Ostatní
hw-report-performance-cores-value = { $count } z { $total }
hw-report-cluster-value = { $name } – fyzická: { $physical }, logická: { $logical }
hw-report-thermal-nominal = Normální teplota
hw-report-thermal-fair = Zahřátý
hw-report-thermal-serious = Horký, zpomaluje
hw-report-thermal-critical = Přehřátý
hw-report-power-battery = Na baterii ({ $percent })
hw-report-power-battery-unknown = Na baterii
hw-report-power-mains = Připojeno k síti
hw-report-power-mains-percent = Připojeno k síti ({ $percent })
hw-report-power-saving = Úspora energie zapnuta
hw-report-gpu-discrete = Samostatný
hw-report-gpu-integrated = Integrovaný
hw-report-gpu-virtual = Virtuální
hw-report-gpu-software = Softwarový, na CPU
hw-report-gpu-other = Jiný
hw-report-gpu-unavailable = Není k dispozici
hw-report-ms = { $ms } ms
hw-report-yes = Ano
hw-report-no = Ne
hw-report-unified-platform-differs = Systém hlásí něco jiného
hw-report-display-value = { $fps } Hz, rezerva HDR nyní { $now }, až { $max }
hw-report-hw-decode-value = { $api } na { $device }
hw-report-hw-decode-checking = Stále se ověřuje…
hw-report-codec-up-to = Až { $width } × { $height }
hw-report-codec-none = Není k dispozici
hw-report-codec-h264-encode = Kódování H.264
hw-report-supported = podporováno: { $list }
hw-report-unsupported = nepodporováno: { $list }
hw-report-calibration-done = Změřeno na tomto počítači
hw-report-calibration-pending = Zatím nezměřeno
hw-report-paths-power-saving = Úspora energie: upřednostňují se cesty s nižší spotřebou energie
hw-report-paths-caption = Cesty pro obrázek s { $mpx } megapixely
hw-report-col-task = Úloha
hw-report-col-path = Cesta
hw-report-col-time = Čas
hw-report-col-cpu = Čas CPU (všechna vlákna)
hw-report-col-runs = Měření
hw-report-col-use = Použití
hw-report-runs-untested = Netestováno
hw-report-use-viewing = Prohlížení ({ $reason })
hw-report-use-background = Na pozadí ({ $reason })
hw-report-reason-only = jediná možnost
hw-report-reason-faster = nejrychlejší
hw-report-reason-cheaper = nejméně času CPU
hw-report-reason-lower-power = nejméně energie
hw-report-reason-explore = zkouší se
hw-report-reason-forced = nastaveno v Nastavení
hw-report-reason-prefer-gpu = Upřednostnit GPU
hw-report-reason-cpu-only = Pouze CPU
hw-report-reason-calibrate = měří se
hw-report-reason-scaled = zmenšená velikost
hw-report-trust-unverified = Zatím neověřeno
hw-report-trust-blocked = Vypnuto po chybě
hw-report-trust-absent = Na tomto počítači není

## Keyboard cheat sheet tabs and sections (machine-drafted, needs native review)
cheatsheet-mode-gallery = Galerie
cheatsheet-mode-compare = Porovnání
cheatsheet-gallery-browse = Procházení
cheatsheet-gallery-select = Výběr
cheatsheet-gallery-mark = Hodnocení a příznaky
cheatsheet-gallery-move = Přechod mezi obrázky
cheatsheet-gallery-page = O stránku nahoru nebo dolů
cheatsheet-gallery-ends = První nebo poslední obrázek
cheatsheet-gallery-up = Nahoru do nadřazené složky
cheatsheet-gallery-history-back = Zpět do předchozí složky
cheatsheet-gallery-history-forward = Vpřed do další složky
cheatsheet-gallery-back = Zavřít horní zobrazení, pak zpět
cheatsheet-gallery-viewer = Zpět do prohlížeče
cheatsheet-gallery-album-add = Přidat do posledního alba
cheatsheet-gallery-select-one = Vybrat nebo zrušit výběr
cheatsheet-gallery-quick-look = Rychlý náhled
cheatsheet-gallery-filter = Filtrovat
cheatsheet-gallery-bigger = Větší miniatury
cheatsheet-gallery-smaller = Menší miniatury
cheatsheet-gallery-default-size = Standardní velikost miniatur
cheatsheet-gallery-refresh = Obnovit složku
cheatsheet-gallery-menu = Místní nabídka
cheatsheet-compare-views = Porovnání
cheatsheet-compare-overlay = Překryv nebo vedle sebe
cheatsheet-compare-difference = Rozdíl (znovu: další druh)
cheatsheet-compare-step-down = Méně prolnutí nebo zesílení rozdílu
cheatsheet-compare-step-up = Více prolnutí nebo zesílení rozdílu
cheatsheet-compare-pick = Obrázek, na který klávesy působí
cheatsheet-compare-close = Zavřít porovnání
cheatsheet-channels = Kanály
cheatsheet-medical-window = Okno a úroveň
cheatsheet-medical-measure = Měření a poznámky
cheatsheet-medical-frames = Snímky a série
# machine-drafted, needs native review
cheatsheet-medical-3d = 3D a rekonstrukce
cheatsheet-medical-analysis = Segmentace, fúze a PET
# machine-drafted, needs native review
cheatsheet-medical-studies = Studie a sdílení
cheatsheet-palette-only-title = V paletě příkazů ({ $key })
cheatsheet-palette-only-title-nokey = V paletě příkazů
cheatsheet-palette-only-tag = Paleta
cheatsheet-palette-only-line = { $action }: bez klávesy; v paletě příkazů ({ $key })
cheatsheet-palette-only-line-nokey = { $action }: bez klávesy; v paletě příkazů
# Esc v přehledu zkratek (dlouhý popis z palety se nevejde).
cheatsheet-back = Zavřít nebo se vrátit

## Hardware video encoding (machine-drafted, needs native review)
hw-report-path-video-encode = Export videa
hw-report-engine-hw-encoder = hardwarový kodér

## Path timing units (machine-drafted, needs native review)
hw-report-task-thumbnail = { $task } (na jednu miniaturu { $size })
hw-report-task-frame = { $task } (na snímek { $size })

## Hardware report, software-only formats (machine-drafted, needs native review)
hw-report-codec-software = Pouze softwarově

about-natural-earth = Mapová data z Natural Earth (naturalearthdata.com), volné dílo.
app-info-show-on-map = Zobrazit na mapě
