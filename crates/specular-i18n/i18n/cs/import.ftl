# machine-drafted, needs native review
# Specular — Import z fotoaparátů, telefonů a paměťových karet. Čeština.
# Karta se jen čte; soubory se z ní mažou jen na požádání, po ověřené kopii.

palette-import = Importovat fotky…
disabled-no-import-source = Není připojena žádná paměťová karta, fotoaparát ani telefon

## Nastavení → Import
settings-section-import = Import
settings-import-on-card = Po vložení karty
settings-import-on-card-help = Co se stane po připojení paměťové karty, fotoaparátu nebo telefonu s fotkami.
settings-import-on-card-ask = Zeptat se
settings-import-on-card-open = Otevřít Import
settings-import-on-card-nothing = Nic nedělat
settings-import-destination = Importovat do
settings-import-destination-help = Složka pro importované fotky a videa. Není-li nastavena, jdou do složky Obrázky.
settings-import-destination-unset = Složka Obrázky
settings-import-folder-pattern = Složky
settings-import-folder-pattern-help = Složky vytvořené uvnitř: {"{"}year{"}"}, {"{"}month{"}"}, {"{"}day{"}"}, {"{"}camera{"}"} a {"{"}event{"}"}, oddělené znakem /.
settings-import-rename-pattern = Přejmenovat
settings-import-rename-pattern-help = Nové názvy souborů, jako v Převést: {"{"}name{"}"}, {"{"}n:03{"}"}, {"{"}date:%Y%m%d{"}"}, {"{"}camera{"}"}, {"{"}event{"}"}. Prázdné ponechá názvy z fotoaparátu.
settings-import-keep-pairs = Ponechat RAW a JPEG spolu
settings-import-keep-pairs-help = Soubor RAW a JPEG pořízený s ním jdou do jedné složky pod jedním názvem, i po přejmenování.
settings-import-low-res-videos = Importovat kopie videí v nízkém rozlišení
settings-import-low-res-videos-help = Některé fotoaparáty ukládají ke každému videu malou náhledovou kopii (.LRV). Vypnuto: zůstanou na kartě.
settings-import-eject = Po dokončení vysunout
settings-import-eject-help = Vysune kartu, jakmile je vše zkopírováno, aby ji šlo vyjmout.

## Nabídka po vložení karty
import-offer = Importovat z { $name }?
import-offer-button = Importovat

## Zobrazení importu
import-title = Import
import-from = z { $name }
import-source-next = Další zdroj
import-close = Zavřít
import-scanning = Hledání nových fotek… { $done } z { $total }
import-scanning-start = Hledání nových fotek…
import-none-new = Vše na { $name } už bylo importováno.
import-empty = Na { $name } nejsou žádné fotky ani videa.
import-no-source = Není připojena žádná paměťová karta, fotoaparát ani telefon.
import-summary = { $count ->
    [one] { $count } nová položka
    [few] { $count } nové položky
    [many] { $count } nové položky
   *[other] { $count } nových položek
} · { $size }
import-already = { $count } už importováno
import-unreadable = { $count ->
    [one] { $count } soubor nelze přečíst
    [few] { $count } soubory nelze přečíst
    [many] { $count } souboru nelze přečíst
   *[other] { $count } souborů nelze přečíst
}
import-selected = Vybráno { $count } z { $total }
import-select-all = Vybrat vše
import-select-none = Zrušit výběr
import-badge-pair = RAW+JPEG
import-badge-raw = RAW
import-badge-video = Video

import-destination = Importovat do
import-choose = Vybrat…
import-folder-pattern = Složky
import-event = Událost
import-event-hint = Volitelné, pro {"{"}event{"}"}
import-rename = Přejmenovat
import-rename-hint = Prázdné ponechá názvy z fotoaparátu
import-preview = První soubor: { $path }
import-pattern-error = { $error }
import-keep-pairs = Ponechat RAW a JPEG spolu
import-eject = Po dokončení vysunout
import-delete = Po importu smazat z karty
import-free = Ke zkopírování { $size } · volné místo { $free }
import-free-unknown = Ke zkopírování { $size }
import-no-space = V { $folder } není dost volného místa: import potřebuje { $need } a volné je jen { $free }. Uvolněte místo nebo vyberte jinou složku.
import-start = { $count ->
    [one] Importovat { $count } položku
    [few] Importovat { $count } položky
    [many] Importovat { $count } položky
   *[other] Importovat { $count } položek
}
import-nothing-selected = Vyberte fotky k importu

import-confirm-delete = { $count ->
    [one] Po zkopírování a ověření bude z { $name } smazán { $count } soubor. Nelze to vrátit zpět.
    [few] Po zkopírování a ověření budou z { $name } smazány { $count } soubory. Nelze to vrátit zpět.
    [many] Po zkopírování a ověření bude z { $name } smazáno { $count } souboru. Nelze to vrátit zpět.
   *[other] Po zkopírování a ověření bude z { $name } smazáno { $count } souborů. Nelze to vrátit zpět.
}
import-confirm-yes = Importovat a smazat
import-confirm-no = Zpět

import-progress = Import { $done } z { $total } · { $size }
import-progress-eta = Import { $done } z { $total } · { $size } · { $time }
import-paused = Pozastaveno · { $done } z { $total }
import-pause = Pozastavit
import-resume = Pokračovat
import-cancel = Zrušit
import-hide = Skrýt
import-stopping = Zastavování…
import-running = Import už probíhá.

import-done = { $count ->
    [one] Importován { $count } soubor
    [few] Importovány { $count } soubory
    [many] Importováno { $count } souboru
   *[other] Importováno { $count } souborů
} do { $folder }
import-done-existed = { $count ->
        [one] { $count } už existoval
        [few] { $count } už existovaly
        [many] { $count } už existovalo
       *[other] { $count } už existovalo
    }
import-done-deleted = { $count } smazáno z karty
import-done-failed = { $count ->
    [one] { $count } soubor nelze importovat
    [few] { $count } soubory nelze importovat
    [many] { $count } souboru nelze importovat
   *[other] { $count } souborů nelze importovat
}
import-done-cancelled = Import zrušen po { $count ->
        [one] { $count } souboru
        [few] { $count } souborech
        [many] { $count } souboru
       *[other] { $count } souborech
    }. Nový import naváže tam, kde skončil.
import-done-nothing = Nic nového nebylo importováno.
import-ejected = { $name } lze vyjmout.
import-eject-failed = { $name } nelze vysunout: { $error }
import-failed-destination = Složku { $folder } nelze použít: { $error }
import-ledger-failed = Seznam importovaných souborů nelze přečíst ({ $error }); dříve importované soubory se mohou nabídnout znovu.

## Čtečka obrazovky
import-a11y-group = Import
import-a11y-tile = { $name }, { $date }
import-a11y-tile-selected = { $name }, { $date }, vybráno
