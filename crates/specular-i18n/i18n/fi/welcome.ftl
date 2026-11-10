# machine-drafted, needs native review
# Specular — Aloitusnäkymä (kun Specular avataan ilman tiedostoa ja kun viimeinen kuva
# suljetaan), sen ”Kokeile näitä” -kortit ja kerran näytettävät vinkit. Suomi.

## Aloitusnäkymä

welcome-title = { -app-name }
welcome-tagline = Nopea ja tarkka katseluohjelma valokuville sekä teknisille, tieteellisille ja lääketieteellisille kuville.
welcome-open = Avaa tiedosto…
welcome-open-about = Kuva, asiakirja tai datatiedosto.
welcome-browse = Selaa kansiota…
welcome-browse-about = Valitse kansio ja näe se galleriana.
welcome-pictures = Omat kuvat
welcome-pictures-about = Kaikki kirjastosi kuvat päivämäärän ja paikan mukaan.
welcome-pictures-setup-title = Luo kirjasto…
welcome-pictures-setup = Aloita kansiosta { $folder }.
welcome-pictures-ask = Lisätäänkö kansio { $folder } kirjastoon?
welcome-pictures-add = Lisää kansio
welcome-drop = Voit myös pudottaa tiedostoja tai kansioita mihin tahansa tähän ikkunaan.
welcome-recent = Viimeisimmät kansiot
welcome-recent-name = { $name }, kansiossa { $parent }
welcome-tips = Kokeile näitä
welcome-tips-hide = Piilota vinkit
welcome-tips-hidden = Vinkit piilotettu. ”Näytä tervetuloa” komentopaletissa tuo ne takaisin.

## Kokeile näitä: kullakin nimi ja yksi rivi. Nimen vieressä on käyttäjän oma näppäin.

welcome-tip-gallery = Galleria
welcome-tip-gallery-about = Näe koko kansio kerralla.
welcome-tip-compare = Vertailu
welcome-tip-compare-about = Valitse galleriasta kahdesta neljään kuvaa nähdäksesi ne vierekkäin.
welcome-tip-cull = Karsintatila
welcome-tip-cull-about = Valitse, hylkää ja arvioi kuvasarja yhdellä näppäimellä kukin.
welcome-tip-slideshow = Diaesitys
welcome-tip-slideshow-about = Näytä kansio koko näytöllä kuva kerrallaan.
welcome-tip-palette = Komentopaletti
welcome-tip-palette-about = Löydä mikä tahansa komento tai asetus nimellä.
welcome-tip-studies = Tutkimukset
welcome-tip-studies-about = DICOM-levyn tai -kansion potilaat, tutkimukset ja sarjat.
welcome-tip-dicom-servers = DICOMweb-palvelimet
welcome-tip-dicom-servers-about = Selaa ja avaa tutkimuksia PACSista.
welcome-tip-dicom-tags = DICOM-tagit
welcome-tip-dicom-tags-about = Kuvan kaikki tagit, haettavissa.
welcome-tip-window-level = Ikkunointi
welcome-tip-window-level-about = Säädä DICOM-kuvan kontrastia vetämällä.
welcome-tip-omero-servers = OMERO-palvelimet
welcome-tip-omero-servers-about = Selaa mikroskopiaprojekteja OMEROssa ja IDR:ssä.
welcome-tip-stretch = Venytys
welcome-tip-stretch-about = Tuo esiin himmeät yksityiskohdat tähtitieteellisessä ja tieteellisessä datassa.
welcome-tip-drawing-layers = Piirustuksen tasot
welcome-tip-drawing-layers-about = Näytä ja piilota teknisen piirustuksen tasot.
welcome-tip-needs-image = Avaa ensin kuva ja paina sitten { $key }.
welcome-tip-needs-gallery = Valitse galleriasta kahdesta neljään kuvaa ja paina sitten { $key }.
welcome-tip-key = Pikanäppäin: { $key }

## Kertavinkit: kukin kerran, huomaamattomasti, enintään yksi käynnistystä kohden.
## Asetukset › Yleiset › Näytä vinkit poistaa ne kaikki käytöstä.

hint-gallery = Paina { $key } nähdäksesi tämän kansion galleriana.
hint-gallery-first = Valitse kahdesta neljään kuvaa ja paina { $compare } vertaillaksesi niitä tai { $cull } karsiaksesi kansion.
hint-raw = Raw-kuvien kansio: paina { $key } karsiaksesi sen (valitse, hylkää ja arvioi).
hint-pages =
    { $count ->
        [one] Tiedostossa on { $count } sivu. { $next } ja { $prev } vaihtavat sivua.
       *[other] Tiedostossa on { $count } sivua. { $next } ja { $prev } vaihtavat sivua.
    }
hint-turn-off = Poista vinkit käytöstä
hint-turned-off = Vinkit poistettu käytöstä. Asetukset › Yleiset › Näytä vinkit ottaa ne takaisin käyttöön.

## Komentopaletti

palette-show-welcome = Näytä tervetuloa
palette-show-welcome-help = Aloitusnäkymä: Avaa, Selaa, omat kuvat ja kokeiltavaa.
palette-close = Sulje
palette-close-help = Sulje kuva tai kansio ja palaa aloitusnäkymään. Aloitusnäkymässä: lopeta.

## Ensikäynnistys (profiilikysymyksen alla)

profile-on-start = Kun { -app-name } avataan ilman tiedostoa:
profile-skip = Ohita
