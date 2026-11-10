# machine-drafted, needs native review
## Optional decoders: other vendors' free software that opens a few formats (Blackmagic RAW,
## DWG), used only when the user installed it. Specular never bundles or downloads it.
## $format is the format ("Blackmagic RAW", "DWG"), $decoder the decoder's own name
## ("Blackmagic RAW SDK", "ODA File Converter"), $vendor its maker ("Blackmagic Design",
## "Open Design Alliance"): names, left as they are.

## Settings › Optional decoders. The switches and paths are shown in the panel below
## "Look again", not as rows of their own.

settings-section-decoders = Valinnaiset dekooderit
settings-decoders-look-again = Etsi uudelleen
settings-decoders-look-again-help = Etsii sen jälkeen asennetut dekooderit.
    { -app-name } etsii myös aina, kun sen ikkuna tulee etualalle.
settings-decoders-braw = Käytä Blackmagic RAW SDK:ta
settings-decoders-braw-help = Blackmagic RAW -leikkeet avataan sillä, kun se on asennettu.
settings-decoders-braw-path = Blackmagic RAW SDK
settings-decoders-braw-path-help = Jätä tyhjäksi, niin se etsitään sieltä, minne asennusohjelma sen asentaa.
settings-decoders-dwg = Käytä ODA File Converteria
settings-decoders-dwg-help = DWG-piirustukset avataan sillä, kun se on asennettu.
    Piirustuksen kopio muunnetaan yksityisessä kansiossa, joka sitten poistetaan.
settings-decoders-dwg-path = ODA File Converter -ohjelma
settings-decoders-dwg-path-help = Jätä tyhjäksi, niin se etsitään sieltä, minne asennusohjelma sen asentaa.
settings-decoders-occt = Käytä OpenCASCADEa
settings-decoders-occt-help = STEP- ja IGES-mallit avataan sillä, kun se on asennettu.
settings-decoders-occt-path = OpenCASCADE-kirjastot
settings-decoders-occt-path-help = Jätä tyhjäksi, niin ne etsitään järjestelmän kirjastojen sijainnista.
# machine-drafted, needs native review
settings-decoders-czi = Käytä libCZI:tä
settings-decoders-czi-help = Zeiss CZI -tiedostot avataan sillä, kun se on asennettu.
    Se toimii { -app-name } -sovelluksen hiekkalaatikoidussa apuprosessissa ilman pääsyä muihin tiedostoihisi.
settings-decoders-czi-path = libCZI-kirjaston kansio
settings-decoders-czi-path-help = Jätä tyhjäksi, niin se etsitään sieltä, minne järjestelmä tallentaa kirjastot.

## The panel: one block per decoder.

decoders-intro = Jotkin muodot avautuvat valmistajiensa ilmaisilla ohjelmistoilla.
    Asenna ohjelmisto kerran: { -app-name } löytää sen ja käyttää sitä. Se ei koskaan lataa sitä itse.
decoders-keywords = valinnaiset dekooderit valmistaja ohjelmisto koodekki laajennus tarvitsee dekooderin optional decoders vendor SDK codec plugin Blackmagic RAW BRAW RED R3D DWG ODA File Converter OpenCASCADE STEP IGES Zeiss CZI libCZI needs decoder
# A decoder's row: "Blackmagic RAW (.braw)". The tick box before it switches it on and off.
decoders-row = { $format } ({ $extensions })
# The row's value.
decoders-state-found = Löytyi
decoders-state-off = Pois
decoders-state-not-found = Ei asennettu
decoders-state-broken = Ei käytettävissä
decoders-state-refused = Ei mahdollinen
decoders-state-planned = Ei vielä saatavilla
decoders-state-checking = Etsitään…
# The row's lines.
decoders-uses = Avautuu ohjelmistolla { $decoder } (valmistaja: { $vendor }).
decoders-not-found = Se on ilmainen. Asenna se ja paina sitten Etsi uudelleen.
decoders-off = Pois käytöstä: nämä tiedostot eivät avaudu.
decoders-version = Versio { $version }.
decoders-r3d-why = REDin lisenssi sallii R3D-tiedostojen lukemisen vain ohjelmille, jotka on tehty sen SDK:lla REDin omin ehdoin.
    { -app-name } ei voi hyväksyä näitä ehtoja, joten R3D-tiedostot eivät avaudu.
decoders-ecw-why = Hexagonin lisenssi sallii ECW SDK:n vain ohjelmille, jotka toimitetaan sen kanssa, ei itse asentamallesi kopiolle.
    Siksi ECW-tiedostot eivät avaudu. ECW SDK:lla tehdyt JPEG 2000 -tiedostot avautuvat.
decoders-mrsid-why = MrSID SDK:n lisenssi sallii sen vain ohjelmille, jotka toimitetaan sen kanssa, ei itse asentamallesi kopiolle.
    Siksi MrSID-tiedostot eivät avaudu.
# machine-drafted, needs native review
# DJI radiometric JPEGs (drone thermal cameras): the photo and metadata open; the
# temperatures would need DJI's SDK.
decoders-dji-thermal-why = DJI:n SDK:n lisenssi sallii sen vain ohjelmille, jotka toimitetaan sen kanssa, ei itse asentamallesi kopiolle. Kuva ja sen metatiedot avautuvat silti.
decoders-occt-why = STEP- ja IGES-kappaleet tarvitsevat OpenCASCADEn, jota tämä { -app-name } -versio ei voi käyttää.
# Rows with one button each, under a decoder.
decoders-get-row = Valmistajan lataussivu
decoders-get = Hae
decoders-locate-row = Asennettu muualle?
decoders-locate = Paikanna…
decoders-forget-row = Valitsit tämän itse.
decoders-forget = Unohda…
# Asked before Forget… does anything.
decoders-forget-ask = Unohdetaanko itse valitsemasi { $decoder }?
decoders-forget-detail = { -app-name } etsii sen taas itse. Levyltä ei poisteta mitään.
decoders-forget-ok = Unohda
# The title of the Locate… file dialog.
decoders-locate-title = Paikanna { $decoder }
# Toasts.
decoders-locate-wrong = Tämä ei ole { $decoder }.
decoders-located = Käytetään valitsemaasi ohjelmistoa { $decoder }.
decoders-forgotten = { -app-name } etsii taas itse ohjelmistoa { $decoder }.
decoders-still-missing = Ohjelmistoa { $decoder } ei vieläkään löydy.
decoders-now-found = { $decoder } löytyi.
decoders-looked = Dekooderit etsittiin uudelleen.

## The card shown in place of a file that needs one. $file is the file's name.

decoders-card-title = { $format } tarvitsee ilmaisen dekooderin (valmistaja: { $vendor })
decoders-card-what = { $file } avautuu ohjelmistolla { $decoder }. Se on ilmainen.
decoders-card-how = Asenna se ja palaa sitten: { -app-name } löytää sen itse.
decoders-card-get = Hae
decoders-card-installed = Asensin sen
decoders-card-locate = Paikanna…

## The gallery: the badge on such a file's thumbnail, and a line of its tooltip.

gallery-badge-needs-decoder = Tarvitsee dekooderin
gallery-needs-decoder = Avautuu ilmaisella ohjelmistolla { $decoder } (valmistaja: { $vendor }), jota ei ole asennettu.

## Why a file did not open, after "Can't open FILE:".

error-reason-needs-decoder = { $format } -tiedostot tarvitsevat ilmaisen ohjelmiston { $decoder } (valmistaja: { $vendor }), jota ei ole asennettu.
