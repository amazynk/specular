# Specular — Kundgallerier: en statisk HTML-mapp med foton, och att läsa tillbaka kundens val.

# machine-drafted, needs native review

palette-client-gallery = Kundgalleri…
palette-client-gallery-help = Spara fotona som en webbsidesmapp till en kund, med val som kunden kan skicka tillbaka.
palette-import-client-picks = Importera kundens val…
palette-import-client-picks-help = Markera fotona som en kund valde i sitt galleri.

webgallery-heading = Kundgalleri
webgallery-subtitle = { $count ->
    [one] Ett foto
   *[other] { $count } foton
}
webgallery-title-label = Rubrik
webgallery-title-default = Korrektur
webgallery-watermark-label = Vattenmärkestext (tom: inget)
webgallery-source = Foton
webgallery-source-current = Det här fotot
webgallery-source-selection = Markering
webgallery-source-album = Albumet ”{ $name }”
webgallery-source-search = Sökresultat
webgallery-source-folder = Den här mappen
webgallery-source-picks = Valda
webgallery-source-value = { $what } ({ $count })
webgallery-theme = Tema
webgallery-theme-light = Ljust
webgallery-theme-dark = Mörkt
webgallery-theme-minimal = Avskalat
webgallery-size = Långsida
webgallery-size-value = { $px } px
webgallery-format = Format
webgallery-captions = Under varje foto
webgallery-captions-off = Inget
webgallery-captions-name = Filnamn
webgallery-captions-title = Rubrik
webgallery-captions-all = Filnamn, rubrik och bildtext
webgallery-exif = Visa kamera och exponering
webgallery-location = Behåll platsen (GPS) i fotona
webgallery-note-private = Ett statiskt galleri kan inte skyddas med lösenord.
webgallery-note-share = Den som har mappen eller dess adress ser alla foton. Dela länken bara med din kund, eller använd webbhotellets åtkomstkontroll.
webgallery-note-picks = Kunder väljer med ett hjärta. Valen stannar i deras webbläsare tills de skickar listan till dig.
webgallery-write = Skapa galleri…
webgallery-busy = Skriver { $done } av { $total }…
webgallery-stop = Avbryt
webgallery-done = { $count ->
    [one] Galleriet skapat: ett foto i { $folder }
   *[other] Galleriet skapat: { $count } foton i { $folder }
}
webgallery-done-failed = { $failed ->
    [one] Ett foto kunde inte skrivas.
   *[other] { $failed } foton kunde inte skrivas.
}
webgallery-stopped = Galleriet avbröts. Inget sparades.
webgallery-failed = Galleriet skapades inte: { $reason }
webgallery-nothing = Inget foto kunde skrivas.

webgallery-page-count = { $count ->
    [one] Ett foto
   *[other] { $count } foton
}
webgallery-page-hint = Tryck på hjärtat på de foton du vill ha. Dina val stannar i den här webbläsaren tills du skickar dem.
webgallery-page-picked = Valda: {"{"}n{"}"}
webgallery-page-pick = Välj
webgallery-page-copy = Kopiera mina val
webgallery-page-copied = Kopierat. Klistra in listan i ett meddelande.
webgallery-page-copy-by-hand = Kopiera den markerade listan och klistra in den i ett meddelande.
webgallery-page-download-txt = Ladda ned picks.txt
webgallery-page-download-json = Ladda ned picks.json
webgallery-page-clear = Rensa val
webgallery-page-clear-ask = Rensa alla dina val i den här webbläsaren?
webgallery-page-no-storage = Den här webbläsaren sparar inte dina val. Skicka dem innan du stänger sidan.
webgallery-page-noscript = För att välja krävs JavaScript. Fotona öppnas ändå.
webgallery-page-previous = Föregående
webgallery-page-next = Nästa
webgallery-page-close = Stäng
webgallery-page-position = {"{"}i{"}"} av {"{"}n{"}"}
webgallery-page-viewer = Fotovisare
webgallery-page-picks = Dina val

clientpicks-heading = Importera kundens val
clientpicks-subtitle = { $count ->
    [one] Jämfört med ett visat foto
   *[other] Jämfört med de { $count } visade fotona
}
clientpicks-paste-label = Namn (klistra in kundens lista)
clientpicks-choose = Välj fil med val…
clientpicks-from-file = Från { $name }
clientpicks-apply-as = Markera dem
clientpicks-as-pick = Som valda
clientpicks-as-stars = { $stars ->
    [one] En stjärna
   *[other] { $stars } stjärnor
}
clientpicks-as-label = Etikett { $label }
clientpicks-empty = Klistra in listan kunden skickade, eller välj kundens picks.txt eller picks.json.
clientpicks-matched = { $matched } av { $total } namn matchar foton här.
clientpicks-missing = Hittades inte: { $names }
clientpicks-ambiguous = I mer än en mapp, lämnade orörda: { $names }
clientpicks-more = { $names } och { $more } till
clientpicks-apply = Verkställ
clientpicks-applied = { $count ->
    [one] Ett foto markerat.
   *[other] { $count } foton markerade.
}
clientpicks-unchanged = De fotona var redan markerade.
clientpicks-read-failed = Kan inte läsa { $name }: { $reason }
