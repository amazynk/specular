# Specular — Galerie pro klienty: statická složka HTML s fotografiemi a načtení výběru klienta zpět.

# machine-drafted, needs native review

palette-client-gallery = Galerie pro klienta…
palette-client-gallery-help = Uložit fotografie jako složku webové stránky pro klienta, s výběrem, který může poslat zpět.
palette-import-client-picks = Importovat výběr klienta…
palette-import-client-picks-help = Označit fotografie, které klient vybral ve své galerii.

webgallery-heading = Galerie pro klienta
webgallery-subtitle = { $count ->
    [one] Jedna fotografie
    [few] { $count } fotografie
    [many] { $count } fotografie
   *[other] { $count } fotografií
}
webgallery-title-label = Název
webgallery-title-default = Náhledy
webgallery-watermark-label = Text vodoznaku (prázdný: žádný)
webgallery-source = Fotografie
webgallery-source-current = Tato fotografie
webgallery-source-selection = Výběr
webgallery-source-album = Album „{ $name }“
webgallery-source-search = Výsledky hledání
webgallery-source-folder = Tato složka
webgallery-source-picks = Vybrané
webgallery-source-value = { $what } ({ $count })
webgallery-theme = Motiv
webgallery-theme-light = Světlý
webgallery-theme-dark = Tmavý
webgallery-theme-minimal = Střídmý
webgallery-size = Delší strana
webgallery-size-value = { $px } px
webgallery-format = Formát
webgallery-captions = Pod každou fotografií
webgallery-captions-off = Nic
webgallery-captions-name = Název souboru
webgallery-captions-title = Název
webgallery-captions-all = Název souboru, název a popisek
webgallery-exif = Zobrazit fotoaparát a expozici
webgallery-location = Ponechat polohu (GPS) ve fotografiích
webgallery-note-private = Statickou galerii nelze chránit heslem.
webgallery-note-share = Kdokoli, kdo má složku nebo její adresu, uvidí všechny fotografie. Sdílejte odkaz jen s klientem, nebo použijte řízení přístupu svého webhostingu.
webgallery-note-picks = Klienti vybírají srdíčkem. Výběr zůstane v jejich prohlížeči, dokud vám seznam nepošlou.
webgallery-write = Vytvořit galerii…
webgallery-busy = Zapisuje se { $done } z { $total }…
webgallery-stop = Zastavit
webgallery-done = { $count ->
    [one] Galerie vytvořena: jedna fotografie v { $folder }
    [few] Galerie vytvořena: { $count } fotografie v { $folder }
    [many] Galerie vytvořena: { $count } fotografie v { $folder }
   *[other] Galerie vytvořena: { $count } fotografií v { $folder }
}
webgallery-done-failed = { $failed ->
    [one] Jednu fotografii nebylo možné zapsat.
    [few] { $failed } fotografie nebylo možné zapsat.
    [many] { $failed } fotografie nebylo možné zapsat.
   *[other] { $failed } fotografií nebylo možné zapsat.
}
webgallery-stopped = Galerie zastavena. Nic nebylo ponecháno.
webgallery-failed = Galerie nebyla vytvořena: { $reason }
webgallery-nothing = Nebylo možné zapsat žádnou fotografii.

webgallery-page-count = { $count ->
    [one] Jedna fotografie
    [few] { $count } fotografie
    [many] { $count } fotografie
   *[other] { $count } fotografií
}
webgallery-page-hint = Klepněte na srdíčko u fotografií, které chcete. Váš výběr zůstane v tomto prohlížeči, dokud ho neodešlete.
webgallery-page-picked = Vybráno: {"{"}n{"}"}
webgallery-page-pick = Vybrat
webgallery-page-copy = Kopírovat můj výběr
webgallery-page-copied = Zkopírováno. Vložte seznam do zprávy.
webgallery-page-copy-by-hand = Zkopírujte označený seznam a vložte ho do zprávy.
webgallery-page-download-txt = Stáhnout picks.txt
webgallery-page-download-json = Stáhnout picks.json
webgallery-page-clear = Vymazat výběr
webgallery-page-clear-ask = Vymazat celý váš výběr v tomto prohlížeči?
webgallery-page-no-storage = Tento prohlížeč váš výběr neuchová. Odešlete ho před zavřením stránky.
webgallery-page-noscript = Vybírání vyžaduje JavaScript. Fotografie se přesto otevřou.
webgallery-page-previous = Předchozí
webgallery-page-next = Další
webgallery-page-close = Zavřít
webgallery-page-position = {"{"}i{"}"} z {"{"}n{"}"}
webgallery-page-viewer = Prohlížeč fotografií
webgallery-page-picks = Váš výběr

clientpicks-heading = Importovat výběr klienta
clientpicks-subtitle = { $count ->
    [one] Porovnáno s jednou zobrazenou fotografií
    [few] Porovnáno se { $count } zobrazenými fotografiemi
    [many] Porovnáno se { $count } zobrazenými fotografiemi
   *[other] Porovnáno s { $count } zobrazenými fotografiemi
}
clientpicks-paste-label = Názvy (vložte seznam klienta)
clientpicks-choose = Vybrat soubor výběru…
clientpicks-from-file = Ze souboru { $name }
clientpicks-apply-as = Označit je
clientpicks-as-pick = Jako vybrané
clientpicks-as-stars = { $stars ->
    [one] Jedna hvězdička
    [few] { $stars } hvězdičky
    [many] { $stars } hvězdičky
   *[other] { $stars } hvězdiček
}
clientpicks-as-label = Štítek { $label }
clientpicks-empty = Vložte seznam, který klient poslal, nebo vyberte jeho picks.txt či picks.json.
clientpicks-matched = { $matched } z { $total } názvů odpovídá zdejším fotografiím.
clientpicks-missing = Nenalezeno: { $names }
clientpicks-ambiguous = Ve více složkách, ponecháno beze změny: { $names }
clientpicks-more = { $names } a { $more } dalších
clientpicks-apply = Použít
clientpicks-applied = { $count ->
    [one] Označena jedna fotografie.
    [few] Označeny { $count } fotografie.
    [many] Označeno { $count } fotografie.
   *[other] Označeno { $count } fotografií.
}
clientpicks-unchanged = Tyto fotografie už byly označené.
clientpicks-read-failed = Soubor { $name } nelze přečíst: { $reason }
