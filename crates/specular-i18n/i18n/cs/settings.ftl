# machine-drafted, needs native review
# Specular — okno Nastavení. Čeština.
#
# Schéma identifikátorů, generované ze schématu nastavení v crates/specular-settings:
#   settings-section-<section>              název oddílu
#   settings-<section>-<key>                popisek nastavení
#   settings-<section>-<key>-help           nápověda pod popiskem
#   settings-<section>-<key>-<value>        jedna možnost nastavení s výběrem
# Klíče a hodnoty jsou názvy ze settings.toml, kde „_“ je zapsáno jako „-“.

## Prvky okna

settings-title = Nastavení
settings-search-placeholder = Hledat v nastavení
settings-search-no-results = Hledání { $query } neodpovídá žádné nastavení
settings-reset = Obnovit
settings-reset-section = Obnovit oddíl
settings-modified = Změněno
settings-default-value = Výchozí: { $value }
settings-restart-required = Projeví se po restartu
settings-restart-now = Restartovat nyní
settings-undo = Zpět
settings-redo = Znovu
settings-value-on = Zapnuto
settings-value-off = Vypnuto
settings-value-auto = Automaticky
settings-value-system = Podle systému
settings-value-system-language = Výchozí jazyk systému
settings-value-follow-language = Podle jazyka rozhraní
settings-value-default-location = Výchozí umístění
settings-choose = Zvolit…
settings-clear = Vymazat
settings-file-reloaded = Soubor settings.toml byl změněn mimo aplikaci { -app-name } a byl znovu načten.
settings-warning-unknown-key = Neznámé nastavení { $key } v souboru settings.toml. Zůstává v souboru, ale ignoruje se.
settings-warning-invalid-value = Neplatná hodnota pro { $key }: { $reason } Místo ní se použije výchozí hodnota.
settings-warning-newer-version = Soubor settings.toml zapsala novější verze aplikace { -app-name }. Nastavení, která tato verze nezná, zůstanou zachována.
settings-warning-syntax = Soubor settings.toml nelze přečíst (řádek { $line }): { $reason } Do jeho opravy se použijí výchozí hodnoty.

## Chyby ověření (vypisuje je i `specular --set`)

settings-error-unknown-key = Nastavení { $key } neexistuje.
settings-error-unknown-key-suggestion = Nastavení { $key } neexistuje. Měli jste na mysli { $suggestion }?
settings-error-not-bool = Očekává se zapnuto nebo vypnuto (true nebo false).
settings-error-not-integer = Očekává se celé číslo.
settings-error-not-number = Očekává se číslo.
settings-error-out-of-range = Musí být mezi { $min } a { $max }.
settings-error-out-of-range-auto = Musí být 0 (automaticky) nebo mezi { $min } a { $max }.
settings-error-unknown-option = Očekává se jedna z možností: { $options }.
settings-error-bad-colour = Očekává se barva, například #1E1E1E nebo #1E1E1E80.
settings-error-bad-language = Očekává se značka jazyka, například de, pt-BR nebo zh-Hans, nebo „system“.
settings-error-bad-path = Očekává se cesta k souboru nebo složce.
settings-error-not-settable = Tato položka je příkaz a nemá hodnotu.
settings-error-syntax = Očekává se klíč=hodnota, například viewing.wheel_behaviour=navigate.
# machine-drafted, needs native review
settings-error-bad-text = Nesmí obsahovat tabulátory, zalomení řádků ani jiné řídicí znaky.
settings-error-invalid = { $reason }
settings-error-tile-url = Je třeba adresa http nebo https s {"{"}z{"}"}, {"{"}x{"}"} a {"{"}y{"}"}.

## Jednotky

unit-percent = { PERCENT($value) }
unit-milliseconds = { $value } ms
unit-seconds = { $value } s
unit-mebibytes = { $value } MiB
# machine-drafted, needs native review
unit-luminance = { $value } cd/m²
unit-images =
    { $value ->
        [one] { $value } obrázek
        [few] { $value } obrázky
        [many] { $value } obrázku
       *[other] { $value } obrázků
    }
unit-threads =
    { $value ->
        [one] { $value } vlákno
        [few] { $value } vlákna
        [many] { $value } vlákna
       *[other] { $value } vláken
    }
unit-bytes =
    { $value ->
        [one] { $value } bajt
        [few] { $value } bajty
        [many] { $value } bajtu
       *[other] { $value } bajtů
    }
unit-kilobytes = { $value } kB
unit-megabytes = { $value } MB
unit-gigabytes = { $value } GB
unit-terabytes = { $value } TB
unit-kibibytes = { $value } KiB
unit-gibibytes = { $value } GiB
unit-tebibytes = { $value } TiB
unit-symbol-mm = mm
unit-symbol-cm = cm
unit-symbol-m = m
unit-symbol-km = km
unit-symbol-mm2 = mm²
unit-symbol-cm2 = cm²
unit-symbol-m2 = m²
unit-symbol-km2 = km²
unit-symbol-ha = ha
unit-symbol-ft = ft
unit-symbol-ft2 = ft²
unit-symbol-mi = mi
unit-symbol-mi2 = mi²
unit-symbol-ac = ac

## Oddíly

settings-section-general = Obecné
settings-section-viewing = Zobrazení
settings-section-browsing = Procházení
settings-section-colour = Barvy
settings-section-metadata = Metadata a soukromí
# machine-drafted, needs native review
settings-section-privacy = Soukromí
settings-section-performance = Výkon
settings-section-accessibility = Usnadnění přístupu
settings-section-keys = Klávesy
settings-section-advanced = Pokročilé

## Obecné

settings-general-language = Jazyk rozhraní
settings-general-language-help = Změny se projeví okamžitě. „Výchozí jazyk systému“ odpovídá jazyku operačního systému.
settings-general-region-formats = Místní formáty
settings-general-region-formats-help = Jak se zapisují data, časy, čísla a velikosti. Ve výchozím nastavení se řídí jazykem rozhraní.
settings-general-theme = Motiv
settings-general-theme-help = Světlý nebo tmavý vzhled oken a ovládacích prvků. Obrázky se nikdy nemění.
settings-general-theme-system = Podle systému
settings-general-theme-light = Světlý
settings-general-theme-dark = Tmavý
# machine-drafted, needs native review
settings-general-on-start = Když se { -app-name } otevře bez souboru
settings-general-on-start-help = Co zobrazit, když se { -app-name } otevře samostatně. Soubor otevřený ze správce souborů nebo z příkazové řádky se vždy zobrazí v prohlížeči.
settings-general-on-start-start-screen = Úvodní obrazovka
settings-general-on-start-gallery = Galerie poslední složky
settings-general-on-start-library = Knihovna
settings-general-start-tips = Zobrazovat „Vyzkoušejte“ na úvodní obrazovce
settings-general-start-tips-help = Řada karet s užitečnými funkcemi a jejich klávesami. „Zobrazit uvítání“ v paletě příkazů ji vrátí.
settings-general-tips = Zobrazovat tipy
settings-general-tips-help = Krátký tip, jednou, když může funkce poprvé pomoci, například klávesa, která zobrazí složku jako galerii. Nikdy během prezentace.
settings-general-tips-shown = Již zobrazené tipy
settings-general-tips-shown-help = Jednorázové tipy, které již byly zobrazeny, aby se neopakovaly.
# machine-drafted, needs native review
settings-general-your-name = Vaše jméno
settings-general-your-name-help = Zobrazuje se na razítkách a anotacích, které přidáte. Ukládá se jen do souborů anotací, které vytvoříte.
settings-general-single-instance = Otevírat soubory ve stávajícím okně
settings-general-single-instance-help = Když { -app-name } již běží, otevření dalšího obrázku ho zobrazí v tomto okně, místo aby se spustila nová instance.
settings-general-file-associations = Přidružení souborů…
settings-general-file-associations-help = Nastavit { -app-name } jako výchozí prohlížeč pro podporované formáty obrázků.

## Zobrazení

settings-viewing-initial-zoom = Počáteční přiblížení
settings-viewing-initial-zoom-help = Jak velký se nově otevřený obrázek zobrazí.
settings-viewing-initial-zoom-fit = Přizpůsobit oknu
settings-viewing-initial-zoom-fit-if-larger = Přizpůsobit velké obrázky, malé zobrazit na { PERCENT(100) }
settings-viewing-initial-zoom-actual-size = Skutečná velikost ({ PERCENT(100) })
settings-viewing-initial-zoom-fill = Vyplnit okno
settings-viewing-wheel-behaviour = Kolečko myši
settings-viewing-wheel-behaviour-help = Co dělá otáčení kolečkem. Podržením Ctrl provedete druhou akci.
settings-viewing-wheel-behaviour-help-macos = Co dělá otáčení kolečkem. Podržením ⌘ provedete druhou akci.
settings-viewing-wheel-behaviour-zoom = Přiblížení
settings-viewing-wheel-behaviour-navigate = Další a předchozí obrázek
settings-viewing-keep-zoom = Zachovat přiblížení mezi obrázky
settings-viewing-keep-zoom-help = Při přiblížení se další a předchozí obrázek otevře se stejným přiblížením a ukáže stejnou část snímku: hodí se ke kontrole ostrosti v sérii.
settings-viewing-zoom-animation = Animovat přiblížení
settings-viewing-zoom-animation-help = Plynulé přibližování místo skoků. Automaticky se vypne, je-li zapnuto „Omezit pohyb“.
settings-viewing-zoom-animation-speed = Rychlost animace přiblížení
settings-viewing-zoom-animation-speed-help = Jak rychle se animované přiblížení ustálí.
settings-viewing-zoom-animation-speed-relaxed = Pozvolná
settings-viewing-zoom-animation-speed-normal = Normální
settings-viewing-zoom-animation-speed-fast = Rychlá
settings-viewing-background = Pozadí
settings-viewing-background-help = Barva za obrázkem v okně.
settings-viewing-background-theme = Podle motivu
settings-viewing-background-black = Černá
settings-viewing-background-dark-grey = Tmavě šedá
settings-viewing-background-light-grey = Světle šedá
settings-viewing-background-white = Bílá
settings-viewing-background-custom = Vlastní barva
settings-viewing-background-color = Vlastní barva pozadí
settings-viewing-background-color-help = Použije se, když je Pozadí nastaveno na „Vlastní barva“.
settings-viewing-fullscreen-background = Pozadí na celé obrazovce
settings-viewing-fullscreen-background-help = Barva za obrázkem na celé obrazovce.
settings-viewing-transparency = Průhledné oblasti
settings-viewing-transparency-help = Jak se vykreslují průhledné části obrázku.
settings-viewing-transparency-checkerboard = Šachovnice
settings-viewing-transparency-background = Barva pozadí
settings-viewing-transparency-black = Černá
settings-viewing-transparency-white = Bílá
settings-viewing-pixel-grid-threshold = Zobrazit mřížku pixelů od
settings-viewing-pixel-grid-threshold-help = Vykreslit mřížku mezi pixely při přiblížení alespoň na tuto úroveň.
settings-viewing-smoothing-high-zoom = Zvětšené pixely
settings-viewing-smoothing-high-zoom-help = Jak pixely vypadají, když je obrázek zobrazen větší než { PERCENT(100) }.
settings-viewing-smoothing-high-zoom-auto = Vyhlazené, od { PERCENT(400) } ostré
settings-viewing-smoothing-high-zoom-smooth = Vždy vyhlazené
settings-viewing-smoothing-high-zoom-pixelated = Vždy ostré čtverce
settings-viewing-double-click = Dvojité kliknutí na obrázek
settings-viewing-double-click-help = Dvojité kliknutí na prázdné pozadí vždy přepne celou obrazovku.
settings-viewing-double-click-toggle-zoom = Přepnout mezi přizpůsobením a { PERCENT(100) }
settings-viewing-double-click-fullscreen = Přepnout celou obrazovku
settings-viewing-double-click-nothing = Nedělat nic
settings-viewing-fullscreen-on-open = Otevírat obrázky na celé obrazovce
settings-viewing-fullscreen-on-open-help = Spustit na celé obrazovce, když se obrázek otevře mimo aplikaci { -app-name }.
settings-viewing-fullscreen-mode = Styl celé obrazovky
settings-viewing-fullscreen-mode-help = Nativní celá obrazovka používá vlastní plochu (Space); okamžitá celá obrazovka překryje aktuální plochu bez animace.
settings-viewing-fullscreen-mode-native = Nativní (vlastní plocha)
settings-viewing-fullscreen-mode-borderless = Okamžitá
settings-viewing-toolbar-autohide = Automaticky skrývat panel nástrojů
settings-viewing-toolbar-autohide-help = Panel nástrojů a šipky po stranách zmizí, když se ukazatel nehýbe, a vrátí se, když se pohne.
settings-viewing-toolbar-autohide-delay = Skrýt panel nástrojů po
settings-viewing-toolbar-autohide-delay-help = Jak dlouho musí být ukazatel v klidu, než panel nástrojů zmizí.
settings-viewing-toolbar-items = Tlačítka panelu nástrojů
settings-viewing-toolbar-items-help = Která tlačítka panel nástrojů zobrazuje a v jakém pořadí.
settings-viewing-toolbar-drag = Uspořádat panel nástrojů přetažením
settings-viewing-toolbar-drag-help = Tlačítko přesunete přetažením po panelu nástrojů. Krátké kliknutí ho stále stiskne; Esc vrátí přetahované tlačítko zpět.
settings-viewing-toolbar-items-open = Otevřít
settings-viewing-toolbar-items-prev = Předchozí obrázek
settings-viewing-toolbar-items-next = Další obrázek
settings-viewing-toolbar-items-zoom-out = Oddálit
settings-viewing-toolbar-items-zoom-level = Úroveň přiblížení
settings-viewing-toolbar-items-zoom-in = Přiblížit
settings-viewing-toolbar-items-fit = Přizpůsobit oknu
settings-viewing-toolbar-items-rotate-left = Otočit proti směru hodinových ručiček
settings-viewing-toolbar-items-rotate-right = Otočit po směru hodinových ručiček
settings-viewing-toolbar-items-info = Informace
settings-viewing-toolbar-items-copy = Kopírovat obrázek
settings-viewing-toolbar-items-delete = Přesunout do koše
settings-viewing-toolbar-items-palette = Paleta příkazů
settings-viewing-toolbar-items-settings = Nastavení
settings-viewing-toolbar-items-fullscreen = Celá obrazovka
# machine-drafted, needs native review
settings-viewing-toolbar-items-spacer = Pružná mezera
settings-viewing-toolbar-items-separator = Oddělovač
settings-toolbar-preview = Náhled panelu nástrojů
settings-toolbar-hidden = Skrytá tlačítka
settings-toolbar-add-spacer = Přidat pružnou mezeru
settings-toolbar-add-separator = Přidat oddělovač
settings-toolbar-reset = Obnovit výchozí
settings-toolbar-remove = Odebrat: { $item }
settings-toolbar-hint = Řádek přesunete přetažením, nebo ho vyberte a stiskněte Alt+↑ či Alt+↓. Tab přepíná mezi polem hledání a seznamem.
settings-toolbar-move-up = Posunout nahoru
settings-toolbar-move-down = Posunout dolů
settings-toolbar-move-start = Posunout na začátek
settings-toolbar-move-end = Posunout na konec
settings-toolbar-menu-hide = Skrýt
settings-toolbar-menu-show = Zobrazit
settings-toolbar-menu-remove = Odebrat
settings-toolbar-row-menu = Další akce: { $item }
settings-toolbar-position = Pozice { $index } z { $count }
settings-toolbar-hidden-state = Skryto

## Procházení

settings-browsing-sort-order = Řadit obrázky podle
settings-browsing-sort-order-help = Pořadí při procházení složky. Řazení podle názvu dá „img2“ před „img10“ a řídí se abecedou vašeho jazyka.
settings-browsing-sort-order-natural = Název
settings-browsing-sort-order-date-taken = Datum pořízení
settings-browsing-sort-order-modified = Datum změny
settings-browsing-sort-order-size = Velikost souboru
settings-browsing-sort-order-type = Typ souboru
settings-browsing-sort-descending = Obrácené pořadí
settings-browsing-sort-descending-help = Řadit od posledního k prvnímu.
settings-browsing-loop-folder = Na konci složky pokračovat od začátku
settings-browsing-loop-folder-help = Po posledním obrázku pokračovat prvním a naopak.
settings-browsing-include-subfolders = Zahrnout podsložky
settings-browsing-include-subfolders-help = Procházet i obrázky v podsložkách.
settings-browsing-hold-acceleration = Zrychlení při podržení další/předchozí
settings-browsing-hold-acceleration-help = Jak rychle se listování zrychluje, když je klávesa nebo tlačítko podrženo.
settings-browsing-hold-acceleration-off = Bez zrychlení
settings-browsing-hold-acceleration-gentle = Mírné
settings-browsing-hold-acceleration-normal = Normální
settings-browsing-hold-acceleration-fast = Rychlé
settings-browsing-confirm-trash = Ptát se před přesunem do koše
settings-browsing-confirm-trash-help = Přesun do koše lze vždy vrátit, proto se { -app-name } ve výchozím nastavení neptá.
settings-browsing-arrows-follow-reading-direction = Šipky podle směru čtení
settings-browsing-arrows-follow-reading-direction-help = V jazycích psaných zprava doleva přejde šipka doleva na další obrázek.

## Barvy

settings-colour-management = Správa barev
settings-colour-management-help = Zobrazovat barvy tak, jak mají vypadat, pomocí barevného profilu obrázku a profilu displeje.
settings-colour-display-profile = Profil displeje
settings-colour-display-profile-help = Barevný profil displeje. Automaticky použije profil, který hlásí operační systém.
settings-colour-rendering-intent = Způsob vykreslení
settings-colour-rendering-intent-help = Jak se barvy, které displej neumí zobrazit, převedou do jeho rozsahu.
settings-colour-rendering-intent-perceptual = Perceptuální
settings-colour-rendering-intent-relative-colorimetric = Relativní kolorimetrický
settings-colour-rendering-intent-saturation = Sytost
settings-colour-rendering-intent-absolute-colorimetric = Absolutní kolorimetrický
settings-colour-black-point-compensation = Kompenzace černého bodu
settings-colour-black-point-compensation-help = Zachovat detaily ve stínech, když je černá displeje světlejší než černá obrázku.
settings-colour-untagged-images = Obrázky bez barevného profilu
settings-colour-untagged-images-help = Jak zobrazit obrázky, které neuvádějí, jaké barvy používají.
settings-colour-untagged-images-srgb = Považovat za sRGB
settings-colour-untagged-images-display = Zobrazit beze změny
settings-colour-dithering = Rozptylování (dithering)
settings-colour-dithering-help = Přidat jemný modrý šum při snižování barevné hloubky, což zabrání pruhům na obloze a v přechodech.
settings-colour-hdr-output = Výstup HDR
settings-colour-hdr-output-help = Zobrazovat obrázky HDR v plném jasu na displejích, které to podporují. Automaticky použije HDR, když displej hlásí vyšší jas; jinak se jasné oblasti šetrně stlačí.
settings-colour-hdr-output-auto = Automaticky
settings-colour-hdr-output-on = Zapnuto
settings-colour-hdr-output-off = Vypnuto
# machine-drafted, needs native review
settings-colour-ocio-config = Konfigurace OCIO
settings-colour-ocio-config-help = Konfigurace OpenColorIO (.ocio) pro obrázky EXR, Radiance HDR a PFM. Prázdné použije tu, kterou určuje $OCIO.
settings-colour-ocio-display = Displej OCIO
settings-colour-ocio-display-help = Displej konfigurace OCIO, který odpovídá vašemu monitoru.
settings-colour-ocio-display-default = První z konfigurace
settings-colour-ocio-view = Pohled OCIO
settings-colour-ocio-view-help = Jak se obrázky scene-linear zobrazí na tomto displeji, například pohledem s mapováním tónů.
settings-colour-ocio-view-default = První z displeje
settings-colour-ocio-input = Vstupní barevný prostor OCIO
settings-colour-ocio-input-help = Barevný prostor hodnot obrázků.
settings-colour-ocio-input-default = Role scene-linear z konfigurace
# machine-drafted, needs native review
settings-colour-proof-profile = Profil softproofu
settings-colour-proof-profile-help = Profil tiskárny, který softproof ve Vyvolání použil naposledy.
settings-colour-proof-intent = Záměr softproofu
settings-colour-proof-intent-help = Jak se převedou barvy, které tiskárna nevytiskne.
settings-colour-proof-intent-perceptual = Perceptuální
settings-colour-proof-intent-relative = Relativní kolorimetrický
settings-colour-proof-bpc = Kompenzace černého bodu softproofu
settings-colour-proof-bpc-help = Namapuje černou na černou tiskárny.
settings-colour-proof-paper = Bílá papíru v softproofu
settings-colour-proof-paper-help = Ukáže vlastní barvu papíru.
settings-colour-proof-ink = Černá inkoustu v softproofu
settings-colour-proof-ink-help = Ukáže, jak tmavá je černá inkoustu.

## Metadata a soukromí

settings-metadata-show-info-on-open = Při otevření zobrazit panel informací
settings-metadata-show-info-on-open-help = Při každém otevření obrázku otevřít panel s údaji o fotoaparátu, datu a místě.
settings-metadata-info-detail = Panel informací zobrazuje
settings-metadata-info-detail-help = Souhrn srozumitelně vysvětlí nejužitečnější údaje; všechna pole vypíší vše, co je v souboru uloženo.
settings-metadata-info-detail-summary = Souhrn
settings-metadata-info-detail-all = Všechna pole
settings-metadata-reverse-geocoding = Zobrazovat názvy míst pro polohy GPS
settings-metadata-reverse-geocoding-help = Vyhledat nejbližší obec ve vestavěných datech. Funguje offline: polohy nikdy neopustí počítač.
settings-metadata-hide-gps = Skrýt polohy GPS na obrazovce
settings-metadata-hide-gps-help = Nezobrazovat souřadnice ani místa, například při sdílení obrazovky. Soubory se nemění.
# machine-drafted, needs native review
settings-metadata-develop-sidecar = Ukládat úpravy z vyvolání vedle fotografie
# machine-drafted, needs native review
settings-metadata-develop-sidecar-help = Zapíše IMG_1234.CR2.specular.xmp vedle souboru, takže úpravy putují s ním. Vypnuto: úpravy zůstanou jen v knihovně.
# machine-drafted, needs native review
settings-metadata-develop-import-lightroom = Importovat nastavení Lightroomu ve Vyvolání
# machine-drafted, needs native review
settings-metadata-develop-import-lightroom-help = Fotka s nastavením Lightroomu a bez úpravy se otevře s ním, přibližně. Vypnuto: Vyvolání nabídne tlačítko.
settings-metadata-units = Jednotky
settings-metadata-units-help = Jednotky nadmořské výšky, vzdáleností a rychlostí.
settings-metadata-units-auto = Podle místních formátů
settings-metadata-units-metric = Metrické
settings-metadata-units-imperial = Imperiální
# machine-drafted, needs native review
settings-metadata-basemap = Podkladová mapa
settings-metadata-basemap-help = Mapa pod polohami fotek. Online mapy stahují dlaždice pro prohlíženou oblast.
settings-metadata-basemap-off = Jen vestavěná mapa (offline)
settings-metadata-basemap-openstreetmap = OpenStreetMap (online)
settings-metadata-basemap-custom = Jiný server dlaždic (online)
settings-metadata-basemap-file = Vlastní mapový soubor
settings-metadata-basemap-url = Adresa serveru dlaždic
settings-metadata-basemap-url-help = Adresa s {"{"}z{"}"}, {"{"}x{"}"} a {"{"}y{"}"}, například https://tiles.example.org/{"{"}z{"}"}/{"{"}x{"}"}/{"{"}y{"}"}.png
settings-metadata-basemap-path = Mapový soubor
settings-metadata-basemap-path-help = Soubor MBTiles nebo PMTiles. Funguje offline.
settings-metadata-basemap-cache = Mezipaměť mapových dlaždic
settings-metadata-basemap-cache-help = Místo na disku pro stažené dlaždice. Nejstarší se mažou první.
# machine-drafted, needs native review
settings-metadata-basemap-allowed = Povolené servery dlaždic
settings-metadata-basemap-allowed-help = Mapové servery, ze kterých jste souhlasili stahovat dlaždice.

## Výkon

settings-performance-ram-cache = Mezipaměť v paměti
settings-performance-ram-cache-help = Paměť pro uchování dekódovaných obrázků připravených k zobrazení. Automaticky zvolí podle velikosti paměti počítače.
settings-performance-vram-cache = Mezipaměť v paměti GPU
settings-performance-vram-cache-help = Paměť GPU pro obrázky a dlaždice. Automaticky zvolí podle GPU.
settings-performance-prefetch-depth = Obrázky připravované předem
settings-performance-prefetch-depth-help = Kolik obrázků ve směru procházení se dekóduje předem.
settings-performance-decoder-threads = Vlákna dekodéru
settings-performance-decoder-threads-help = Vlákna CPU pro dekódování obrázků. Automaticky použije všechna výkonná jádra a přizpůsobí se souboru.
settings-performance-gpu-adapter = GPU
settings-performance-gpu-adapter-help = Které GPU použít, má-li počítač více než jedno.
settings-performance-gpu-adapter-high-performance = Nejvyšší výkon
settings-performance-gpu-adapter-low-power = Nejnižší spotřeba
settings-performance-processing = Kde se obrázky zpracovávají
settings-performance-processing-help = Část práce může běžet na CPU, GPU nebo hardwarovém dekodéru. Automaticky změří tento počítač a zvolí nejrychlejší způsob pro právě prohlížený obrázek a nejúspornější způsob pro práci na pozadí. Co na tomto počítači zvolil a proč, je uvedeno níže.
settings-performance-processing-automatic = Automaticky
settings-performance-processing-prefer-gpu = Upřednostnit GPU
settings-performance-processing-cpu-only = Pouze CPU
settings-performance-drag-preview = Náhled při tažení
settings-performance-drag-preview-help = Jak vyvolání zobrazuje fotografii, když táhnete posuvníkem. Automaticky použije menší obraz, jen když počítač nestíhá.
settings-performance-drag-preview-automatic = Automaticky
settings-performance-drag-preview-sharp = Vždy ostrý
settings-performance-drag-preview-fast = Vždy rychlý

## Nastavení → Výkon → Jak { -app-name } běží na tomto počítači (výběr cest,
## DESIGN.md §12.3.1). $engine a $other jsou názvy hw-report-engine-* (malými písmeny,
## uprostřed věty, všechny rodu mužského); paths-engine-* jsou samostatné názvy.
settings-paths-title = Jak { -app-name } běží na tomto počítači
settings-paths-intro = { -app-name } měří, jak rychle tento počítač zvládá tyto úlohy jednotlivými způsoby, a použije ten nejlepší. Časy platí pro fotografii s { $mpx } megapixely.
settings-paths-intro-measuring = Měření ještě není dokončeno: { -app-name } měří, když je počítač nečinný.
settings-paths-intro-battery = Napájení z baterie: volba upřednostňuje nižší spotřebu energie.
settings-paths-keywords = GPU CPU NPU grafická karta hardwarová akcelerace dekodér benchmark rychlost výkon procesor
paths-engine-cpu = CPU
paths-engine-cpu-parallel = CPU, všechna jádra
paths-engine-gpu = GPU
paths-engine-hw-decoder = Hardwarový dekodér
paths-engine-npu = NPU
settings-paths-your-setting = { $engine } (vaše nastavení)
settings-paths-measured-faster = Změřeno na tomto počítači: { $ratio }× rychleji než { $other }.
settings-paths-measured-similar = Změřeno na tomto počítači: přibližně stejně rychle jako { $other }.
settings-paths-measured-power = Při napájení z baterie spotřebuje nejméně energie (změřeno na tomto počítači).
settings-paths-not-measured = Zatím nezměřeno: obvyklá volba pro tento typ počítače.
settings-paths-set-by = Zvoleno podle nastavení „{ $setting }: { $option }“.
settings-paths-blocked-check = { $engine }: vypnuto { $date }, protože výsledek neodpovídal výsledku CPU.
settings-paths-blocked-error = { $engine }: vypnuto { $date } po chybě.
settings-paths-blocked-lost = { $engine }: vypnuto { $date }, protože přestalo odpovídat.
settings-paths-blocked-undated = { $engine }: vypnuto po chybě.
settings-paths-checking = { $engine } se před použitím pro právě prohlížený obrázek ověřuje proti CPU.
settings-paths-exploring = { $engine } se právě zkouší na pozadí.
settings-paths-unavailable = { $engine } není na tomto počítači pro tuto úlohu k dispozici.
settings-paths-background-cheaper = Na pozadí: { $engine }, podle měření spotřebuje o { $percent } méně času CPU.
settings-paths-background-power = Na pozadí: { $engine }, kvůli úspoře energie.
settings-paths-background = Na pozadí: { $engine }.
settings-paths-detail-measured = { $engine }: { $time } ms, { $cpu } ms času CPU, { $runs ->
    [one] { $runs } měření
    [few] { $runs } měření
    [many] { $runs } měření
   *[other] { $runs } měření
} ({ $confidence })
settings-paths-detail-guess = { $engine }: přibližně { $time } ms (odhad)
settings-paths-confidence-low = nízká spolehlivost
settings-paths-confidence-medium = střední spolehlivost
settings-paths-confidence-high = vysoká spolehlivost
settings-paths-machine = Změřeno pro { $gpu } · { $driver } · { $cpu }
settings-paths-retry = Zkusit znovu
settings-paths-measure-again = Změřit znovu
settings-paths-measure-again-question = Zapomenout údaje změřené na tomto počítači včetně vypnutých cest a změřit znovu, až bude počítač nečinný?
settings-paths-measure-again-confirm = Zapomenout měření
settings-performance-display-cache = Uchovávat mezipaměť zobrazení na disku
settings-performance-display-cache-help = Ukládat kopie obrázků ve velikosti obrazovky, aby se již navštívené složky otevíraly okamžitě.
settings-performance-display-cache-size = Velikost mezipaměti zobrazení
settings-performance-display-cache-size-help = Když je mezipaměť plná, nejstarší položky se odstraní.
settings-performance-display-cache-location = Umístění mezipaměti zobrazení
settings-performance-display-cache-location-help = Nejlépe funguje složka na rychlém disku.
settings-performance-fast-simd = Používat instrukce SIMD
settings-performance-fast-simd-help = Používat k dekódování vektorové instrukce CPU. Vypínejte jen při diagnostice problému.
settings-performance-fast-dct-scaling = Dekódovat ve velikosti obrazovky
settings-performance-fast-dct-scaling-help = Dekódovat velké soubory JPEG přímo ve velikosti, v jaké se zobrazují. Vypínejte jen při diagnostice problému.
settings-performance-fast-parallel-decode = Dekódovat na více jádrech
settings-performance-fast-parallel-decode-help = Rozdělit velké obrázky mezi jádra CPU. Vypínejte jen při diagnostice problému.
settings-performance-fast-restart-index = Indexovat velké soubory JPEG pro přiblížení
settings-performance-fast-restart-index-help = Zapamatovat si, kde začíná každá část souboru JPEG, aby se přiblížené oblasti dekódovaly rychle. Vypínejte jen při diagnostice problému.
settings-performance-fast-yuv-upload = Převádět barvy na GPU
settings-performance-fast-yuv-upload-help = Převádět barvy JPEG na GPU nebo CPU. Automaticky zvolí to, co je na tomto počítači rychlejší. Vypínejte jen při diagnostice problému.
settings-performance-fast-yuv-upload-auto = Automaticky
settings-performance-fast-yuv-upload-on = Zapnuto
settings-performance-fast-yuv-upload-off = Vypnuto
settings-performance-fast-async-io = Asynchronní čtení souborů
settings-performance-fast-async-io-help = Číst dopředu nejrychlejším souborovým rozhraním operačního systému. Vypínejte jen při diagnostice problému.
settings-performance-fast-mmap = Soubory mapované do paměti
settings-performance-fast-mmap-help = Číst soubory bez kopírování. Vypínejte jen při diagnostice problému.
settings-performance-hardware-decode = Hardwarové dekódování
settings-performance-hardware-decode-help = Otevírat fotografie JPEG vestavěným dekodérem GPU. Automaticky ho použije tam, kde pomůže, obvykle u obrázků připravovaných na pozadí. Nejprve se ověří proti běžnému dekodéru a vše, co nezvládne, se otevře běžně.
settings-performance-hardware-decode-auto = Automaticky
settings-performance-hardware-decode-on = Zapnuto
settings-performance-hardware-decode-off = Vypnuto
settings-performance-frame-time-hud = HUD časů snímků
settings-performance-frame-time-hud-help = Zobrazit časy dekódování a snímků a používané rychlé cesty.
settings-performance-stall-warning = Hlásit zaseknuté načítání po
settings-performance-stall-warning-help = Když se obrázek po této době stále načítá, zapsat stav načítání a čekající úlohy do protokolu (standardní chybový výstup nebo SPECULAR_LOG_FILE).
# machine-drafted, needs native review
settings-performance-insights = Tipy k výkonu
settings-performance-insights-help = Když se obrázky opakovaně otevírají pomalu ze stejného důvodu, zobrazovat v rohu „Proč je to pomalé?“ s tím, co by pomohlo. Co se při tom zjistí, zůstává na tomto počítači.
settings-performance-insights-storage = Tipy k pomalému úložišti
settings-performance-insights-storage-help = Zobrazovat tipy, když čtení souborů (ze síťové sdílené složky, pomalé jednotky nebo z archivů) opakovaně zdržuje otevírání obrázků.
settings-performance-insights-decoder = Tipy k dekódování
settings-performance-insights-decoder-help = Zobrazovat tipy, když se obrázky opakovaně dekódují na CPU, přestože by mohl pomoci hardwarový dekodér.
# machine-drafted, needs native review
settings-performance-insights-restore = Znovu zobrazit skryté tipy
settings-performance-insights-restore-help = Vrátí tipy, které jste skryli pomocí × u „Proč je to pomalé?“, pro otevírání obrázků i pro miniatury. Nedostupné, dokud žádné skryté nejsou.

## Usnadnění přístupu

settings-accessibility-reduce-motion = Omezit pohyb
settings-accessibility-reduce-motion-help = Nahradit animace přiblížení, posunu a prolínání okamžitými změnami.
settings-accessibility-reduce-motion-system = Podle systému
settings-accessibility-reduce-motion-on = Zapnuto
settings-accessibility-reduce-motion-off = Vypnuto
settings-accessibility-increase-contrast = Zvýšit kontrast
settings-accessibility-increase-contrast-help = Výraznější okraje a text a menší průsvitnost.
settings-accessibility-increase-contrast-system = Podle systému
settings-accessibility-increase-contrast-on = Zapnuto
settings-accessibility-increase-contrast-off = Vypnuto
settings-accessibility-text-scale = Velikost textu
settings-accessibility-text-scale-help = Velikost textu vzhledem ke standardní velikosti.
settings-accessibility-ui-scale = Měřítko rozhraní
settings-accessibility-ui-scale-help = Velikost celého rozhraní, nad rámec škálování displeje nastaveného v operačním systému.
settings-accessibility-focus-ring-always = Vždy zobrazovat rámeček zaměření
settings-accessibility-focus-ring-always-help = Zobrazovat, který ovládací prvek má fokus klávesnice, i při používání myši.
settings-accessibility-screen-reader-verbosity = Podrobnost čtečky obrazovky
# machine-drafted, needs native review
settings-accessibility-screen-reader-verbosity-help = Kolik toho čtečka obrazovky oznamuje při přechodu mezi obrázky a kolik z údajů pod ukazatelem přečte.
settings-accessibility-screen-reader-verbosity-concise = Stručně
settings-accessibility-screen-reader-verbosity-normal = Normálně
settings-accessibility-screen-reader-verbosity-verbose = Podrobně
# machine-drafted, needs native review
settings-accessibility-announce-cursor = Číst hodnotu pod ukazatelem
settings-accessibility-announce-cursor-help = Čtečka obrazovky přečte hodnotu pixelu, HU, teplotu nebo polohu pod ukazatelem či klávesovým zaměřovačem, jakmile se přestane pohybovat.

## Soukromí

# machine-drafted, needs native review
settings-privacy-no-network = Bez sítě
# machine-drafted, needs native review
settings-privacy-no-network-help = Vypnuto: připojuje se jen toto, každé až po zapnutí nebo otevření něčeho jeho pomocí: kontrola aktualizací, stahování modelů místní AI, online mapy, obrázky z webové adresy nebo cloudového úložiště a servery, které přidáte (DICOMweb, OMERO, mapy). Zapnuto: nepřipojuje se nic.

## Pokročilé

settings-advanced-portable-mode = Přenosný režim
settings-advanced-portable-mode-help = Ukládat nastavení a mezipaměti do složky vedle programu místo do uživatelského profilu.
settings-advanced-log-level = Podrobnost protokolu
settings-advanced-log-level-help = Kolik toho { -app-name } zapisuje do souboru protokolu.
settings-advanced-log-level-error = Jen chyby
settings-advanced-log-level-warn = Varování
settings-advanced-log-level-info = Informace
settings-advanced-log-level-debug = Ladění
settings-advanced-log-level-trace = Vše
settings-advanced-developer-mode = Vývojářský režim
settings-advanced-developer-mode-help = Zobrazit nástroje pro vývoj a testování aplikace { -app-name }, například pseudojazyky v seznamu jazyků.
# machine-drafted, needs native review
settings-advanced-audit-log = Místní protokol auditu
# machine-drafted, needs native review
settings-advanced-audit-log-help = Zaznamenávat otevřené lékařské snímky, exporty, anonymizaci, tisk a odesílání do PACS do souboru audit.jsonl ve složce dat. Zůstává v tomto počítači.
settings-advanced-hw-report = Zpráva o hardwaru…
settings-advanced-hw-report-help = Zobrazit, jaké funkce CPU a GPU { -app-name } rozpoznal a které rychlé cesty používá.
# machine-drafted, needs native review
settings-advanced-renderer-info = Informace o vykreslování…
settings-advanced-renderer-info-help = Zobrazí grafický adaptér, ovladač, formáty zobrazení a limity paměti, které aplikace { -app-name } používá, připravené ke zkopírování do hlášení chyby.
settings-advanced-edit-settings-file = Upravit settings.toml
settings-advanced-edit-settings-file-help = Upravte soubor nastavení ve vestavěném editoru { -app-name }. Kontroluje soubor během psaní a uloží ho, jen když neobsahuje chyby.
settings-advanced-import = Importovat nastavení…
settings-advanced-import-help = Načíst nastavení ze souboru exportovaného z aplikace { -app-name }.
settings-advanced-export = Exportovat nastavení…
settings-advanced-export-help = Uložit všechna nastavení do jednoho souboru jako zálohu nebo pro jiný počítač.
settings-advanced-reset-all = Obnovit všechna nastavení…
settings-advanced-reset-all-help = Vrátit všechna nastavení na výchozí hodnoty. Tuto akci lze vrátit.

## settings.toml editor (machine-drafted, needs native review)

config-editor-title = settings.toml
config-editor-save = Uložit
config-editor-revert = Vrátit
config-editor-close = Zavřít
config-editor-modified = Změněno
config-editor-text = Soubor nastavení
config-editor-position = Řádek { $line }, sloupec { $column }
config-editor-saved = Soubor settings.toml byl uložen a použit.
config-editor-cant-save = Nelze uložit. Problémy k opravě: { $count }.
config-editor-save-failed = Soubor settings.toml nelze uložit: { $reason }
config-editor-conflict = Soubor settings.toml byl změněn jinde.
config-editor-reload = Znovu načíst
config-editor-keep-mine = Ponechat moje
config-editor-discard-question = Zahodit neuložené změny v settings.toml?
config-editor-discard = Zahodit
config-editor-keep-editing = Pokračovat v úpravách
config-editor-find-placeholder = Najít v souboru
config-editor-find-count = { $current } z { $total }
config-editor-find-none = Žádné shody
config-editor-find-previous = Předchozí shoda
config-editor-find-next = Další shoda
config-editor-find-close = Zavřít hledání
config-editor-problems = Problémy
config-editor-problems-count =
    { $count ->
        [0] Žádné problémy
       *[other] Problémy: { $count }
    }
config-editor-no-problems = Žádné problémy. Soubor lze uložit.
config-editor-problem-at = Řádek { $line }: { $message }
config-editor-severity-error = Chyba
config-editor-severity-warning = Upozornění
config-editor-severity-info = Poznámka
config-editor-problem-syntax = Neplatný TOML: { $reason }
config-editor-problem-duplicate = { $key } je ve stejné tabulce nastaveno dvakrát. Jedno odstraňte.
config-editor-problem-unknown-section = Sekce nastavení [{ $key }] neexistuje.
config-editor-problem-unknown-section-suggestion = Sekce nastavení [{ $key }] neexistuje. Měli jste na mysli [{ $suggestion }]?
config-editor-problem-unknown-key = Nastavení { $key } neexistuje.
config-editor-problem-unknown-key-suggestion = Nastavení { $key } neexistuje. Měli jste na mysli { $suggestion }?
config-editor-problem-invalid = Neplatná hodnota pro { $key }: { $reason }
config-editor-problem-migrated = { $key } je zapsáno tak, jak to zapisovala starší verze { -app-name }. Při čtení se převede.
config-editor-problem-newer-version = Tento soubor zapsala novější verze { -app-name } (formát { $found }; tato verze čte { $supported }). Neznámá nastavení se zachovají.
config-editor-problem-command = Příkaz { $index } v [[commands]]: { $reason }
config-editor-problem-hook = Háček { $index } v [[hooks]]: { $reason }

## keymap.toml in the same editor (machine-drafted, needs native review)

config-editor-keymap-title = keymap.toml
config-editor-keymap-text = Soubor klávesových zkratek
config-editor-keymap-saved = keymap.toml uložen a použit.
config-editor-keymap-save-failed = keymap.toml nelze uložit: { $reason }
config-editor-keymap-conflict = keymap.toml se změnil jinde.
config-editor-keymap-discard-question = Zahodit neuložené změny v keymap.toml?
config-editor-new-file = nový soubor, vytvoří se při uložení
config-editor-no-folder = neexistuje složka nastavení, kam by se dal uložit.

## Reset all settings confirmation (machine-drafted, needs native review)

settings-reset-all-confirm = Obnovit všechna nastavení na výchozí hodnoty?
settings-reset-all-confirm-detail = Týká se to všech částí Nastavení. Později to lze vrátit zpět.
settings-reset-all-confirm-ok = Obnovit vše
settings-reset-all-done = Všechna nastavení byla obnovena na výchozí hodnoty.

## Hardware video encoding (machine-drafted, needs native review)
paths-engine-hw-encoder = Hardwarový kodér

## Path timing units (machine-drafted, needs native review)
settings-paths-checking-before-use = { $engine } se před použitím ověřuje proti CPU.
settings-paths-unit-thumbnail = Časy platí pro jednu miniaturu { $size }
settings-paths-unit-frame = Časy platí pro jeden snímek videa { $size }
# machine-drafted, needs native review
settings-metadata-develop-calibration = Složka kalibračních snímků
# machine-drafted, needs native review
settings-metadata-develop-calibration-help = Flat a dark snímky pro Vyvolání. Názvy souborů nebo složek obsahují „flat“ nebo „dark“.

## Colour swatches under a colour setting's field (machine-drafted, needs native review)
settings-colour-white = Bílá
settings-colour-light-grey = Světle šedá
settings-colour-grey = Šedá
settings-colour-dark-grey = Tmavě šedá
settings-colour-black = Černá
settings-colour-yellow = Žlutá
settings-colour-orange = Oranžová
settings-colour-red = Červená
settings-colour-magenta = Purpurová
settings-colour-blue = Modrá
settings-colour-cyan = Azurová
settings-colour-green = Zelená
