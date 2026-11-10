# Specular — Asiakasgalleriat: staattinen HTML-kansio kuvista, ja asiakkaan valintojen lukeminen takaisin.

# machine-drafted, needs native review

palette-client-gallery = Asiakasgalleria…
palette-client-gallery-help = Tallenna kuvat verkkosivukansioksi asiakkaalle, jossa on valinnat, jotka asiakas voi lähettää takaisin.
palette-import-client-picks = Tuo asiakkaan valinnat…
palette-import-client-picks-help = Merkitse kuvat, jotka asiakas valitsi galleriastaan.

webgallery-heading = Asiakasgalleria
webgallery-subtitle = { $count ->
    [one] Yksi kuva
   *[other] { $count } kuvaa
}
webgallery-title-label = Otsikko
webgallery-title-default = Vedokset
webgallery-watermark-label = Vesileiman teksti (tyhjä: ei mitään)
webgallery-source = Kuvat
webgallery-source-current = Tämä kuva
webgallery-source-selection = Valinta
webgallery-source-album = Albumi ”{ $name }”
webgallery-source-search = Hakutulokset
webgallery-source-folder = Tämä kansio
webgallery-source-picks = Valitut
webgallery-source-value = { $what } ({ $count })
webgallery-theme = Teema
webgallery-theme-light = Vaalea
webgallery-theme-dark = Tumma
webgallery-theme-minimal = Pelkistetty
webgallery-size = Pitkä sivu
webgallery-size-value = { $px } px
webgallery-format = Muoto
webgallery-captions = Kunkin kuvan alla
webgallery-captions-off = Ei mitään
webgallery-captions-name = Tiedostonimi
webgallery-captions-title = Otsikko
webgallery-captions-all = Tiedostonimi, otsikko ja kuvateksti
webgallery-exif = Näytä kamera ja valotus
webgallery-location = Säilytä sijainti (GPS) kuvissa
webgallery-note-private = Staattista galleriaa ei voi suojata salasanalla.
webgallery-note-share = Jokainen, jolla on kansio tai sen osoite, näkee kaikki kuvat. Jaa linkki vain asiakkaallesi, tai käytä verkkopalvelusi pääsynhallintaa.
webgallery-note-picks = Asiakkaat valitsevat sydämellä. Valinnat pysyvät heidän selaimessaan, kunnes he lähettävät listan sinulle.
webgallery-write = Luo galleria…
webgallery-busy = Kirjoitetaan { $done }/{ $total }…
webgallery-stop = Pysäytä
webgallery-done = { $count ->
    [one] Galleria luotu: yksi kuva kansiossa { $folder }
   *[other] Galleria luotu: { $count } kuvaa kansiossa { $folder }
}
webgallery-done-failed = { $failed ->
    [one] Yhtä kuvaa ei voitu kirjoittaa.
   *[other] { $failed } kuvaa ei voitu kirjoittaa.
}
webgallery-stopped = Galleria pysäytettiin. Mitään ei säilytetty.
webgallery-failed = Galleriaa ei luotu: { $reason }
webgallery-nothing = Yhtään kuvaa ei voitu kirjoittaa.

webgallery-page-count = { $count ->
    [one] Yksi kuva
   *[other] { $count } kuvaa
}
webgallery-page-hint = Napauta sydäntä haluamissasi kuvissa. Valintasi pysyvät tässä selaimessa, kunnes lähetät ne.
webgallery-page-picked = Valittu: {"{"}n{"}"}
webgallery-page-pick = Valitse
webgallery-page-copy = Kopioi valintani
webgallery-page-copied = Kopioitu. Liitä lista viestiin.
webgallery-page-copy-by-hand = Kopioi valittu lista ja liitä se viestiin.
webgallery-page-download-txt = Lataa picks.txt
webgallery-page-download-json = Lataa picks.json
webgallery-page-clear = Tyhjennä valinnat
webgallery-page-clear-ask = Tyhjennetäänkö kaikki valintasi tästä selaimesta?
webgallery-page-no-storage = Tämä selain ei säilytä valintojasi. Lähetä ne ennen kuin suljet sivun.
webgallery-page-noscript = Valitseminen vaatii JavaScriptin. Kuvat aukeavat silti.
webgallery-page-previous = Edellinen
webgallery-page-next = Seuraava
webgallery-page-close = Sulje
webgallery-page-position = {"{"}i{"}"}/{"{"}n{"}"}
webgallery-page-viewer = Kuvakatselin
webgallery-page-picks = Valintasi

clientpicks-heading = Tuo asiakkaan valinnat
clientpicks-subtitle = { $count ->
    [one] Verrattu yhteen näytettyyn kuvaan
   *[other] Verrattu { $count } näytettyyn kuvaan
}
clientpicks-paste-label = Nimet (liitä asiakkaan lista)
clientpicks-choose = Valitse valintatiedosto…
clientpicks-from-file = Tiedostosta { $name }
clientpicks-apply-as = Merkitse ne
clientpicks-as-pick = Valituiksi
clientpicks-as-stars = { $stars ->
    [one] Yksi tähti
   *[other] { $stars } tähteä
}
clientpicks-as-label = Väritunniste { $label }
clientpicks-empty = Liitä asiakkaan lähettämä lista, tai valitse hänen picks.txt- tai picks.json-tiedostonsa.
clientpicks-matched = { $matched }/{ $total } nimestä vastaa täällä olevia kuvia.
clientpicks-missing = Ei löytynyt: { $names }
clientpicks-ambiguous = Useammassa kuin yhdessä kansiossa, jätetty ennalleen: { $names }
clientpicks-more = { $names } ja { $more } muuta
clientpicks-apply = Käytä
clientpicks-applied = { $count ->
    [one] Yksi kuva merkitty.
   *[other] { $count } kuvaa merkitty.
}
clientpicks-unchanged = Ne kuvat oli jo merkitty.
clientpicks-read-failed = Tiedostoa { $name } ei voi lukea: { $reason }
