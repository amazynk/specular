# Specular — Drawings (DXF): layers, paper, lineweights, model space and layouts. Čeština.

keys-mode-drawing = Výkresy
cheatsheet-drawing = Výkresy
palette-drawing-layers = Zobrazit nebo skrýt panel hladin
palette-drawing-all-layers = Zobrazit všechny hladiny, nebo hladiny podle uložení
palette-drawing-paper = Prohodit tmavý a světlý papír
palette-drawing-lineweights = Zobrazit nebo skrýt tloušťky čar
disabled-not-drawing = Pouze pro výkresy (DXF)
# A drawing's page: model space (layouts are shown by their own name).
page-label-model = Model
# machine-drafted, needs native review
cad-pen-name = Pero { $n }
cad-page-name = Strana { $n }
cad-layer-name = Hladina { $n }
cad-map-name = Mapa
# machine-drafted, needs native review
# A board's views (its Gerber files together): every layer stacked, and its top and bottom
# in board colours.
cad-board-layers-name = Vrstvy
cad-board-top-name = Horní strana
cad-board-bottom-name = Spodní strana
# machine-drafted, needs native review
# A plot's layers: its frame, a trace without a name (numbered from 1), and the contour
# lines of a 2-D spectrum above and below zero.
cad-chart-frame-name = Rámeček
cad-trace-name = Křivka { $n }
cad-contours-positive-name = Kladné vrstevnice
cad-contours-negative-name = Záporné vrstevnice
# machine-drafted, needs native review
# The page of a survey file (LandXML) showing its surfaces and alignments in 3D.
cad-model-3d-name = 3D model
# The layers panel's title (and its name for screen readers).
drawing-layers-title = Hladiny · zobrazeno { $shown } z { $total }
# Toasts.
drawing-layers-all-toast = Zobrazeny všechny hladiny
drawing-layers-saved-toast = Hladiny podle uložení ve výkresu
drawing-paper-dark-toast = Tmavý papír
drawing-paper-light-toast = Světlý papír
drawing-lineweights-on = Tloušťky čar zobrazeny
drawing-lineweights-off = Tloušťky čar vypnuty: tenké čáry

# machine-drafted, needs native review
# Markup on drawings and images (Alt+M): kept in a file next to the drawing, never in it.
palette-markup = Přidat anotace
palette-markup-help = Obláčky, text, šipky, razítka, kóty a špendlíky s fotkou, uložené vedle souboru.
palette-compare-sheets = Porovnat výkresy
palette-compare-sheets-help = Překrýt jinou revizi tohoto výkresu, zarovnanou, se zvýrazněnými změnami.
markup-title = Anotace
markup-list = Anotace na této stránce
markup-count = { $count ->
    [0] Na této stránce nejsou žádné anotace
    [one] 1 anotace na této stránce
    [few] { $count } anotace na této stránce
    [many] { $count } anotace na této stránce
   *[other] { $count } anotací na této stránce
}
markup-empty = Vyberte nástroj a klikněte do výkresu.
markup-item = { $n }. { $kind }
markup-tool-select = Vybrat
markup-tool-cloud = Obláček
markup-tool-text = Text
markup-tool-arrow = Šipka
markup-tool-stamp = Razítko
markup-tool-length = Délka
markup-tool-area = Plocha
markup-tool-pin = Špendlík s fotkou
markup-hint-select = Kliknutím položku vyberete. Tažením ji přesunete. Enter upraví její text.
markup-hint-cloud = Táhněte obdélník, nebo klikejte na rohy a stiskněte Enter.
markup-hint-text = Klikněte tam, kam patří text.
markup-hint-arrow = Táhněte od popisku k bodu, nebo klikněte na obojí.
markup-hint-stamp = Klikněte tam, kam patří razítko.
markup-hint-length = Táhněte, nebo klikněte na oba konce.
markup-hint-area = Klikejte na rohy a pak stiskněte Enter.
markup-hint-pin = Klikněte na bod a pak vyberte fotku.
markup-kind-cloud = Obláček
markup-kind-text = Text
markup-kind-arrow = Šipka
markup-kind-stamp = Razítko
markup-kind-length = Délka
markup-kind-area = Plocha
markup-kind-pin = Fotka
markup-colour = Barva: { $colour }
markup-colour-red = Červená
markup-colour-blue = Modrá
markup-colour-green = Zelená
markup-colour-orange = Oranžová
markup-colour-magenta = Purpurová
markup-colour-black = Černá
markup-width = Čára: { $width }
markup-width-thin = Tenká
markup-width-medium = Střední
markup-width-thick = Silná
markup-stamp = Razítko: { $words }
markup-stamp-approved = Schváleno
markup-stamp-rejected = Zamítnuto
markup-stamp-reviewed = Zkontrolováno
markup-stamp-custom = Vlastní text
markup-undo = Zpět
markup-redo = Znovu
markup-delete = Smazat
markup-open-photo = Otevřít fotku
markup-from-measurements = Přidat měření
markup-from-measurements-done = { $count ->
    [one] 1 měření přidáno jako anotace
    [few] { $count } měření přidána jako anotace
    [many] { $count } měření přidáno jako anotace
   *[other] { $count } měření přidáno jako anotace
}
markup-export-pdf = Exportovat PDF…
markup-export-png = Exportovat PNG…
markup-done = Hotovo
markup-ask-text = Text (Enter umístí, Esc zruší)
markup-ask-label = Popisek (volitelný; Enter umístí)
markup-ask-stamp = Text razítka (Enter umístí)
markup-ask-edit = Nový text (Enter uloží)
markup-added = Přidáno: { $kind }
markup-moved = Přesunuto
markup-pin-hint = Klikněte na špendlík znovu nebo stiskněte Enter a otevře se jeho fotka.
markup-deleted = Smazáno
markup-undone = Vráceno zpět
markup-redone = Provedeno znovu
markup-delete-ask = Smazat { $name }?
markup-delete-detail = Položka se odstraní ze souboru anotací. Samotný výkres se nemění.
markup-delete-ok = Smazat
markup-photo-missing = { $name } už neexistuje.
markup-load-failed = Anotace nenačteny: { $reason }
markup-save-failed = Anotace neuloženy: { $reason }
markup-sidecar-too-large = soubor anotací je příliš velký
markup-pdf-unreadable = PDF nelze přečíst
markup-exported = Uloženo { $name }
markup-export-failed = Neexportováno: { $reason }
markup-export-same-file = zvolte jiný název; originál se nikdy nepřepisuje
markup-export-encrypted = PDF je šifrované. Exportujte raději PNG.
markup-export-damaged = struktura PDF je poškozená. Exportujte raději PNG.
sheets-title = Porovnat výkresy
sheets-other = Jiný výkres…
sheets-other-named = Jiný: { $name }
sheets-page = Strana { $page } z { $count }
sheets-auto = Zarovnat
sheets-pick = Vybrat 2 body
sheets-overlay = Překrytí
sheets-difference = Rozdíl
sheets-blend = Prolínání
sheets-blink = Blikání
sheets-slider = Jiný výkres { PERCENT($percent) }
sheets-clouds = { $count ->
    [0] Žádné změny
    [one] Označit obláčkem 1 změnu
    [few] Označit obláčky { $count } změny
    [many] Označit obláčky { $count } změny
   *[other] Označit obláčky { $count } změn
}
sheets-done = Hotovo
sheets-choose = Vyberte druhý výkres.
sheets-reading = Načítání výkresu…
sheets-aligning = Zarovnávání…
sheets-aligned = Zarovnáno: měřítko { $scale }, otočení { $angle }°, odchylka do { $error } px. { $changes ->
    [0] Žádné změny nenalezeny.
    [one] 1 změna.
    [few] { $changes } změny.
    [many] { $changes } změny.
   *[other] { $changes } změn.
}
sheets-align-failed = Automatické zarovnání se nezdařilo. Vyberte 2 body.
sheets-pick-this = Bod { $n }: klikněte na něj na tomto výkresu.
sheets-pick-other = Bod { $n }: klikněte na stejný bod na druhém výkresu.
sheets-pick-same = Oba body jsou stejné. Vyberte 2 body znovu.
sheets-read-failed = Výkres nenačten: { $reason }
sheets-failed = Tento krok se nezdařil. Zkuste to znovu nebo vyberte jiný výkres.
sheets-legend-this = Jen tento výkres
sheets-legend-other = Jen druhý výkres
sheets-legend-both = Oba
sheets-cloud-text = Změna
sheets-clouds-added = { $count ->
    [0] Nepřidány žádné obláčky
    [one] 1 obláček přidán do anotací
    [few] { $count } obláčky přidány do anotací
    [many] { $count } obláčku přidáno do anotací
   *[other] { $count } obláčků přidáno do anotací
}
sheets-opened = Porovnat výkresy
sheets-closed = Porovnání zavřeno
markup-ask-new-line = Shift+Enter: nový řádek

## Settings › Drawings

# machine-drafted, needs native review
settings-section-drawings = Výkresy
settings-drawings-shx-folders = Složky písem SHX
settings-drawings-shx-folders-help = Složky s písmy SHX pro text v DXF a DWG, oddělené znakem ;. Nejdříve se prohledává složka samotného výkresu.
# A folder in the SHX font folders setting that does not exist or is not a folder.
settings-error-shx-folder = Nejde o složku: { $folder }
