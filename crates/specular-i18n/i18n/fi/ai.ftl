# Specular — Local AI pack (DESIGN.md "Local AI pack", §10). Finnish. Machine-drafted, needs native review.

## Settings → Local AI
settings-section-ai = Paikallinen tekoäly
settings-ai-pack = Paikallinen tekoälypaketti
# machine-drafted, needs native review
settings-ai-pack-help = Valinnaisia toimintoja, jotka toimivat vain tällä tietokoneella. Mikään niiden löytämä ei poistu siltä. Valitse kullekin toiminnolle malli; mitään ei ladata ennen kuin valitset sen.
# machine-drafted, needs native review
settings-ai-install-from-file = Asenna tiedostosta…
settings-ai-install-from-file-help = Asenna tälle tietokoneelle kopioidut mallitiedostot koneisiin, joilla ei ole internetiä. Jokainen tiedosto tarkistetaan { -app-name }in mukana toimitettua luetteloa vasten.
settings-ai-install-from-folder = Asenna kansiosta…
settings-ai-install-from-folder-help = Sama kaikille kansion mallitiedostoille.
settings-ai-search = Haku kuvauksella
settings-ai-search-help = Löydä kuvia kuvailemalla niitä, esimerkiksi ”koira rannalla”, ja ryhmittele samankaltaiset kuvat paremmin.
settings-ai-text = Teksti kuvissa
settings-ai-text-help = Lue kuvien ja skannausten teksti, jotta text:-haku löytää ne.
settings-ai-culling = Karsinta-apu
# machine-drafted, needs native review
settings-ai-culling-help = Merkitse suljetut silmät, epäterävyys ja huono valotus, jotta sarjan paras kuva on helpompi valita.
settings-ai-people = Henkilöt
# machine-drafted, needs native review
settings-ai-people-help = Löydä kasvot ja ryhmittele ne henkilöittäin people:-hakua varten. Ryhmittely on pois päältä, kunnes otat sen käyttöön kirjastossa. Kasvotiedot voi poistaa yhdeltä henkilöltä tai kaikilta kerralla.

# The pack's and a feature's state. $percent is formatted ("25%"); $done and $total are
# formatted numbers.
ai-state = { $state ->
        [none] Tässä versiossa ei ole malleja.
        [installed] Asennettu
        [indexing] Asennettu · { $done }/{ $total } kuvaa indeksoitu
        [downloading] Ladataan… { $percent }
        [failed] Lataus epäonnistui
       *[available] Ei asennettu
    }
ai-removing = Poistetaan…
# machine-drafted, needs native review
# Buttons of the Local AI page.
ai-button = { $action ->
        [download] Lataa
        [cancel] Peruuta
        [remove] Poista
        [forget] Unohda
        [faces] Poista kaikki kasvotiedot
       *[pack] Poista paketti
    }
# The question before deleting.
ai-confirm = { $action ->
        [faces] Poistetaanko kaikki kasvotiedot? Kaikki löydetyt kasvot ja henkilöt poistetaan. Kuvasi säilyvät.
       *[pack] Poistetaanko paikallinen tekoälypaketti, sen mallit ja kaikki sen löytämä? Kuviasi ei muuteta.
    }

## Gallery search
gallery-query-needs-ai = { $field }: vaatii paikallisen tekoälypaketin (Asetukset › Paikallinen tekoäly)
# machine-drafted, needs native review
gallery-query-ai-not-yet = { $field }: paikallinen tekoälypaketti ei vielä osaa hakea tätä

## Faces: People view, face boxes and culling flags
# machine-drafted, needs native review
# "Show faces" on the picture.
faces-shown = Kasvot näytetään
# machine-drafted, needs native review
faces-hidden = Kasvot piilotettu
# machine-drafted, needs native review
# Beside a face box whose eyes look closed.
faces-eyes-closed = Silmät kiinni
# machine-drafted, needs native review
# Culling flags on the cull mode strip.
cull-flag-eyes-closed = Silmät kiinni
# machine-drafted, needs native review
cull-flag-eyes-closed-some = Silmät kiinni: { $closed } / { $faces } kasvoista
# machine-drafted, needs native review
cull-flag-blurred = Epätarkka
# machine-drafted, needs native review
cull-flag-dark = Liian tumma
# machine-drafted, needs native review
cull-flag-bright = Liian vaalea
# machine-drafted, needs native review
cull-flag-clipped = Leikkautuneet kirkkaat sävyt tai varjot
# machine-drafted, needs native review
# The People view.
people-title = Henkilöt
# machine-drafted, needs native review
people-open = Henkilöt
# machine-drafted, needs native review
people-done = Valmis
# machine-drafted, needs native review
people-back = Takaisin
# machine-drafted, needs native review
people-show-photos = Näytä kuvat
# machine-drafted, needs native review
people-name = Nimeä…
# machine-drafted, needs native review
people-name-prompt = Nimeä tämä henkilö
# machine-drafted, needs native review
people-faces-open = Kasvot
# machine-drafted, needs native review
people-faces-of = Kasvot: { $name }
# machine-drafted, needs native review
people-merge = Yhdistä
# machine-drafted, needs native review
people-merged = Henkilöitä yhdistetty: { $count }
# machine-drafted, needs native review
people-merge-ask = Yhdistetäänkö { $from } henkilöön { $into }?
# machine-drafted, needs native review
people-merge-ask-many = Yhdistetäänkö { $count } henkilöä henkilöön { $into }?
# machine-drafted, needs native review
people-merge-ask-detail = Kasvot siirtyvät henkilölle { $into }. Levyn kuvat eivät muutu.
# machine-drafted, needs native review
people-not-this = Ei tämä henkilö
# machine-drafted, needs native review
people-face-removed = Kasvot poistettu tältä henkilöltä. Kuva säilyy.
# machine-drafted, needs native review
# Someone not named yet.
people-unnamed = Henkilö #{ $id }
# machine-drafted, needs native review
people-face-count = { $count ->
        [one] { $count } kasvot
       *[other] { $count } kasvoa
    }
# machine-drafted, needs native review
people-count = { $count ->
        [one] { $count } henkilö
       *[other] { $count } henkilöä
    }
# machine-drafted, needs native review
people-face-a11y = Kasvot { $index }
# machine-drafted, needs native review
people-loading = Ladataan…
# machine-drafted, needs native review
people-not-installed = Henkilöt ei ole asennettu. Lataa se kohdassa Asetukset › Paikallinen tekoäly.
# machine-drafted, needs native review
people-grouping-off = Kasvojen ryhmittely on pois päältä tässä kirjastossa. Ota ”Ryhmittele kasvot henkilöiksi” käyttöön kohdassa Asetukset › Paikallinen tekoäly.
# machine-drafted, needs native review
people-none = Henkilöitä ei ole vielä löytynyt.

# machine-drafted, needs native review
# A feature's model: Default (the model Specular ships as the default, or the nearest one
# this computer can run) or a model by name, which stays that model on any computer. The
# page shows it only for a feature with more than one model; the dropdown labels Default
# with the model it means (ai-size-recommended-option) and names each model as the model
# list does. The option names below are the palette's and the command line's: model names
# are proper names, kept as they are.
settings-ai-search-size = Malli
settings-ai-search-size-help = SigLIP 2 B/16 löytää kuvat parhaiten, monella kielellä. B/32 indeksoi noin neljä kertaa nopeammin, hitaammille näytönohjaimille.
settings-ai-search-size-recommended = Oletus
# machine-drafted, needs native review
settings-ai-search-size-small = SigLIP 2 B/32
settings-ai-search-size-standard = PE Core S16
settings-ai-search-size-large = SigLIP 2 B/16
settings-ai-text-size = Malli
settings-ai-text-size-help = PP-OCRv6 small lukee tarkemmin; tiny on nopeampi.
settings-ai-text-size-recommended = Oletus
settings-ai-text-size-small = PP-OCRv6 tiny
settings-ai-text-size-standard = PP-OCRv6 small
settings-ai-text-size-large = Suuri
# machine-drafted, needs native review
settings-ai-culling-size = Malli
settings-ai-culling-size-help = Malli, joka tunnistaa suljetut silmät.
settings-ai-culling-size-recommended = Oletus
settings-ai-culling-size-small = Pieni
# machine-drafted, needs native review
settings-ai-culling-size-standard = Face Landmarker
settings-ai-culling-size-large = Suuri
# machine-drafted, needs native review
settings-ai-people-size = Malli
settings-ai-people-size-help = SFace on pieni. AuraFace on suurempi, ja se on koulutettu datalla, jonka julkaisija sanoo olevan lisensoitu kaupalliseen käyttöön.
settings-ai-people-size-recommended = Oletus
settings-ai-people-size-small = Pieni
# machine-drafted, needs native review
settings-ai-people-size-standard = BlazeFace + SFace
settings-ai-people-size-large = BlazeFace + AuraFace
# A model size by name.
ai-tier = { $tier ->
        [small] Pieni
        [large] Suuri
       *[standard] Vakio
    }
# machine-drafted, needs native review
# Default in the model dropdown, and the label of the "Back to default" row: with the model
# it means on this computer, "Default (SigLIP 2 B/16)". $size is the model's name.
ai-size-recommended-option = Oletus ({ $size })
# Under the model setting: the default, or the nearest model this computer can run. $gpu is
# the graphics card's name.
ai-recommended-for = Oletus näytönohjaimellesi { $gpu }: { $size }
ai-recommended = Oletus tälle tietokoneelle: { $size }
# Nothing fits: no size can run here.
ai-recommended-none = Mikään koko ei mahdu tämän tietokoneen muistiin.
# Under an installed feature: what its model runs on now. $gpu is the graphics card's name
# (used with "gpu" only). The CPU variants say why the graphics card is not used.
ai-runs-on = { $engine ->
        [gpu] Suoritus: näytönohjain ({ $gpu })
        [gpu-unnamed] Suoritus: näytönohjain
        [no-gpu] Suoritus: CPU (näytönohjainta ei löytynyt)
        [differed] Suoritus: CPU – näytönohjaimen tulos poikkesi CPU:n tuloksesta
        [failed] Suoritus: CPU – malli epäonnistui näytönohjaimella
        [setting] Suoritus: CPU (käsittelyksi on asetettu ”Vain CPU”)
        [checking] Suoritus: CPU – näytönohjainta verrataan siihen
       *[cpu] Suoritus: CPU
    }
# A size's row: its files on disk ($place "disk"), or what it would download, the memory it
# needs, and " · needs the GPU" only for a size that cannot run without one. $bytes and
# $memory are formatted sizes.
ai-size-detail = { $place ->
        [disk] { $bytes } levyllä
       *[download] { $bytes } ladattavaa
    } · { $memory } muistia{ $gpu ->
        [yes] { " · vaatii näytönohjaimen" }
       *[no] { "" }
    }
# machine-drafted, needs native review
# Under a size that may not run on this computer (it is still shown, and choosing it asks
# first). "gpu": it needs a graphics card this computer does not have or cannot use;
# "memory": $need is the memory it needs, $have what this computer has for it (formatted
# sizes, "6 GB").
ai-size-unfit = { $why ->
        [gpu] Vaatii näytönohjaimen, eikä tässä tietokoneessa ole sellaista, jota se voi käyttää.
       *[memory] Vaatii { $need } muistia, enemmän kuin { $have }, jonka Specular sallii tekoälylle tässä tietokoneessa.
    }
# Over the size setting while it names a size other than the one installed and in use
# ($current, $chosen: ai-tier). The rows below have the buttons to change it.
ai-size-not-in-use = Käytössä on { $current }, ei { $chosen }.
# machine-drafted, needs native review
# Buttons of a size's row and of the size question: "use" chooses that size (one not
# downloaded; a downloaded one says Switch, ai-size-button), "back" returns the setting to
# Recommended, "anyway" keeps a size that may not run on this computer (ai-unfit-question).
# The screen reader adds the size's name to each.
ai-size-action = { $action ->
        [back] Takaisin oletukseen
        [anyway] Käytä silti
       *[use] Käytä tätä kokoa
    }
# The size the setting chose is not installed while another one is ($current, ai-tier).
ai-size-chosen-missing = Valittu, mutta ei vielä ladattu: { $current } on yhä käytössä.
# What a size is now: from what is installed, never from the setting. "downloaded": its files
# are here but another size is in use; "missing": the chosen size, not downloaded.
ai-size-state = { $state ->
        [current] Käytössä
        [downloaded] Ladattu, ei käytössä
        [missing] Ei ladattu
        [previous] Käytössä, kunnes uusi koko on indeksoinut kirjastosi
        [target] Ladataan…
        [failed] Lataus epäonnistui
       *[other] { "" }
    }
# A licence row's button (opens the licence's page).
ai-licence = Näytä lisenssi
# Asked before a size change that downloads and re-indexes. $feature is the feature's name,
# $size the new size (ai-tier), $download a formatted size.
ai-switch-question = { $download ->
        [none] { $feature }: vaihdetaanko kokoon { $size }? Kirjastosi indeksoidaan uudelleen; nykyinen koko on käytössä siihen asti.
       *[other] { $feature }: vaihdetaanko kokoon { $size }? Tämä lataa { $download } ja indeksoi kirjastosi uudelleen; nykyinen koko on käytössä siihen asti.
    }
# machine-drafted, needs native review
# A size that may not run on this computer, chosen for a feature not installed yet: asked
# before the setting keeps it (ai-size-unfit says why, on the next line). $size is ai-tier,
# $feature the feature's name.
ai-unfit-question = { $feature }: käytetäänkö kokoa { $size } silti? Se ei ehkä toimi tässä tietokoneessa, tai toimii vain hyvin hitaasti.
# The recommendation moved: offered, never applied silently.
ai-suggest-question = { $feature }: koko { $size } sopii nyt paremmin tälle tietokoneelle. Vaihdetaanko? { $download ->
        [none] Kirjastosi indeksoidaan uudelleen.
       *[other] Tämä lataa { $download } ja indeksoi kirjastosi uudelleen.
    }
ai-suggest-toast = Paikallinen tekoäly: toinen mallin koko sopii paremmin tälle tietokoneelle (Asetukset › Paikallinen tekoäly).
# A feature switching size: the new size ($size, ai-tier) indexes the library while the old one
# stays in use. $done and $total are formatted numbers.
ai-switching = Vaihdetaan kokoon { $size } · { $done }/{ $total } kuvaa indeksoitu

## The model manager: what each model says besides its size and licence

# machine-drafted, needs native review
# A model or feature not installable yet.
ai-coming = Tulossa myöhemmässä versiossa
# machine-drafted, needs native review
# What a model reads. $languages: multilingual, english, or a script id (OCR packs).
ai-choice-languages = { $languages ->
        [multilingual] Monta kieltä
        [english] Vain englanti
        [cjk-latin] Kiina, japani ja latinalainen kirjaimisto
        [zh-latin] Kiina ja latinalainen kirjaimisto
        [arabic] Arabia
        [hebrew] Heprea
        [korean] Korea
        [cyrillic] Kyrillinen kirjaimisto
        [greek] Kreikka
        [thai] Thai
        [devanagari] Devanagari
        [tamil] Tamili
        [telugu] Telugu
        [eslav] Venäjä, ukraina ja valkovenäjä
        [latin] Latinalainen kirjaimisto (monet eurooppalaiset kielet)
       *[other] { $languages }
    }
# A hint about a model. $hint: faster, closed-eyes, small-faces.
ai-choice-hint = { $hint ->
        [faster] Nopeampi, hitaammille näytönohjaimille
        [closed-eyes] Tunnistaa suljetut silmät
        [small-faces] Parempi pienten kasvojen kanssa
        [no-translate] Ei käännä englanniksi
       *[other] { "" }
    }
# A model's training data. $source: the data as its publisher names it ($named is "no" when
# it is not named). $terms: whether the publisher states the data's terms ("stated"), does
# not ("unstated"), or the data is licensed for research only ("research").
ai-choice-data = { $terms ->
        [stated] Koulutusdata: { $source }
        [research] Koulutusdata: { $source } (lisensoitu vain tutkimukseen)
       *[unstated] { $named ->
            [no] Koulutusdata: nimeämätön, ehtoja ei ilmoitettu
           *[yes] Koulutusdata: { $source }, ehtoja ei ilmoitettu
        }
    }
# Where a model downloads from. $host is a host name ("huggingface.co").
ai-choice-host = Ladataan palvelimelta { $host }
# A model still coming, in a feature's row. $size is formatted, $licence an SPDX name.
ai-choice-coming = { $name }: { $size }, { $licence }
# People: grouping faces into people, per library.
ai-grouping = Ryhmittele kasvot henkilöiksi
ai-grouping-help = Pois päältä, kunnes otat sen käyttöön tässä kirjastossa. Poistaminen käytöstä lopettaa ryhmittelyn; jo ryhmitellyt kasvot säilyvät, kunnes poistat ne.
ai-grouping-state = { $on ->
        [yes] Käytössä tässä kirjastossa
       *[no] Pois päältä
    }
ai-grouping-button = { $on ->
        [yes] Poista käytöstä
       *[no] Ota käyttöön
    }
# machine-drafted, needs native review
# An add-on (a script's text recogniser) before its feature is downloaded. $feature is the
# feature's name ("Text in photos").
ai-addon-first = Lataa ensin { $feature }.
# machine-drafted, needs native review
# Asked before an add-on is removed. $name is the model's name ("PP-OCRv5 Arabic").
ai-addon-remove-title = Poistetaanko { $name }?
# machine-drafted, needs native review
ai-addon-remove-detail = Sen mallitiedostot poistetaan. Kuvasi ja jo löydetty teksti säilyvät.
# machine-drafted, needs native review
# Text: reading the text in this library's photos, per library.
ai-text-indexing = Lue teksti tämän kirjaston kuvista
# machine-drafted, needs native review
ai-text-indexing-help = Pois päältä, kunnes otat sen käyttöön tässä kirjastossa. Sen jälkeen text: Kirjaston haussa löytää kuvat niissä olevien sanojen perusteella.
# machine-drafted, needs native review
ai-text-indexing-state = { $on ->
        [yes] Käytössä tässä kirjastossa
       *[no] Pois päältä
    }

## Asennus tiedostosta

# machine-drafted, needs native review
# After model files were copied in.
ai-sideload-ready = { $count ->
        [one] Yksi mallitiedosto kopioitiin. Asenna se painamalla Lataa.
       *[other] { $count } mallitiedostoa kopioitiin. Asenna ne painamalla Lataa.
    }
ai-sideload-none = Mallitiedostoja ei löytynyt.
# $file is a file name, or several separated by commas.
ai-sideload-unknown = { $file } ei ole { -app-name }in tuntema malli. Sitä ei kopioitu.
ai-sideload-failed = Tiedostoa { $file } ei voitu kopioida: { $reason }
# Asked before using a file whose SHA-256 isn't in the shipped list. $model is the model it
# would be used as ("clip-vit-b32-1").
ai-unverified-title = Käytetäänkö vahvistamatonta mallia?
ai-unverified-detail = { $file } ei ole { -app-name }in mukana toimitettujen mallien luettelossa, joten sitä ei voi tarkistaa. Sitä käytetään mallina { $model }.
ai-unverified-use = Käytä vahvistamatonta mallia

# machine-drafted, needs native review
ai-failed-internal = { -app-name }in sisällä meni jotain vikaan. Lokissa on tiedot.
# The buttons under a model-size question: switch, or keep the size in use.
ai-size-button = { $action ->
        [switch] Vaihda
       *[keep] Säilytä nykyinen koko
    }
