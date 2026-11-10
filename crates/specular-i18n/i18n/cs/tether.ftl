# Specular — tethered capture: a camera on USB, its shots downloaded as they are taken. Čeština. machine-drafted, needs native review

palette-tether = Fotografování s kabelem…
palette-tether-capture = Vyfotit (s kabelem)

tether-title = Fotografování s kabelem
tether-close = Zavřít fotografování s kabelem
tether-default-folder = S kabelem

## Cameras
tether-none = Nebyl nalezen žádný fotoaparát.
tether-none-hint = Připojte fotoaparát kabelem USB a zapněte ho. Má-li režim USB, zvolte PTP nebo připojení k počítači.
tether-look-again = Hledat znovu
tether-connect = Připojit
tether-next-camera = Další fotoaparát
tether-connecting = Připojování…
tether-connected = { $camera } připojen
tether-disconnect = Odpojit
tether-try-again = Zkusit znovu
tether-gone = Fotoaparát byl odpojen.
tether-not-connected = Není připojen žádný fotoaparát. Nejprve otevřete fotografování s kabelem.

## Why a camera can't be opened
tether-permission = Tento uživatel nesmí otevřít zařízení USB fotoaparátu { $node }.
tether-permission-rule = Povolí to pravidlo udev. Jako správce vložte tento řádek do { $file }:
tether-permission-reload = Pak spusťte „sudo udevadm control --reload“ a fotoaparát znovu připojte.
tether-copy-rule = Kopírovat pravidlo
tether-busy-other = Fotoaparát používá jiný program.
tether-busy-named = Fotoaparát používá { $programs }.
tether-busy-hint = Zavřete ten program nebo odpojte fotoaparát ve správci souborů a zkuste to znovu.
tether-driver = Systémový ovladač tohoto fotoaparátu nedovolí jiným programům ho používat.

## The camera
tether-battery = Baterie { PERCENT($percent) }
tether-card = { $card }: volno { $free }
tether-card-shots = { $card }: volno { $free }, snímků: { $shots }
tether-no-card = Žádná paměťová karta.
tether-no-settings = Fotoaparát nesděluje nastavení expozice.
tether-no-remote = Tento fotoaparát odsud nelze spustit. Snímky pořízené jeho spouští se stahují.
tether-capture = Vyfotit
tether-step-down = { $setting }: snížit ({ $value })
tether-step-up = { $setting }: zvýšit ({ $value })

## Exposure settings
tether-prop-mode = Režim
tether-prop-shutter = Čas závěrky
tether-prop-aperture = Clona
tether-prop-iso = ISO
tether-prop-bias = Korekce expozice
tether-prop-white-balance = Vyvážení bílé
tether-auto = Automaticky
tether-bulb = Režim B
tether-camera-value = Nastavení fotoaparátu { $value }
tether-wb-manual = Ručně
tether-wb-one-push = Jednorázové měření
tether-wb-daylight = Denní světlo
tether-wb-fluorescent = Zářivka
tether-wb-tungsten = Žárovka
tether-wb-flash = Blesk
tether-mode-manual = Ruční
tether-mode-program = Programová automatika
tether-mode-aperture = Priorita clony
tether-mode-shutter = Priorita času
tether-mode-creative = Kreativní
tether-mode-action = Sport
tether-mode-portrait = Portrét

## Where shots go
tether-folder = Složka
tether-names = Názvy
tether-names-camera = Názvy z fotoaparátu
tether-arrival = Styl Develop
tether-choose-folder = Složka…
tether-choose-style = Styl…
tether-downloading = Stahování… { PERCENT($percent) }
tether-saved = Uloženo { $name }
tether-saved-count = { $count ->
    [one] Uložen { $count } snímek
    [few] Uloženy { $count } snímky
    [many] Uloženo { $count } snímku
   *[other] Uloženo { $count } snímků
}

## Failures
tether-busy = Fotoaparát je zaneprázdněn. Zkuste to znovu.
tether-card-full = Paměťová karta je plná.
tether-set-refused = Fotoaparát nastavení nepřijal. Zkontrolujte volič režimů.
tether-disk = Snímek nelze uložit: { $error }
tether-error = Chyba fotoaparátu: { $error }

## Settings › Cull mode
settings-cull-tether-folder = Složka pro fotografování s kabelem
settings-cull-tether-folder-help = Kam se ukládají snímky z fotoaparátu připojeného kabelem.
settings-cull-tether-folder-unset = Obrázky › S kabelem
settings-cull-tether-rename = Názvy souborů při fotografování s kabelem
settings-cull-tether-rename-help = Nové názvy se stejnými značkami jako Přejmenovat v Importu. Prázdné ponechá názvy z fotoaparátu.
