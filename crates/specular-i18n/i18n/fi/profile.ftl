# Specular — Profiles: what Specular is used for (the first-run question, Settings → Profile,
# the toolbar chip that turns a profile's tools on). Suomi.

settings-section-profile = Profiili
settings-profile-capabilities = Mihin käytän { -app-name }ia
settings-profile-capabilities-help = Jokainen valinta lisää omat työkalunsa: tilat, komennot, työkalurivin painikkeet ja Asetusten sivut. Yleiset on aina käytössä, ja jokainen muoto avautuu valinnasta riippumatta.
settings-profile-capabilities-general = Yleiset
settings-profile-capabilities-engineering = Tekniikka
settings-profile-capabilities-medical = Lääketiede
settings-profile-capabilities-science = Tiede
# machine-drafted, needs native review
settings-profile-capabilities-maps = Kartat (GIS)
settings-profile-shown = Jo tarjottu
settings-profile-shown-help = Ensikäynnistyksen kysymys ja jo näytetyt ”Aina”-tarjoukset, jotta niitä ei näytetä uudelleen.
# The first-run sheet's title; the choices are the settings-profile-capabilities-* labels.
profile-question = Mihin aiot käyttää { -app-name }ia?
profile-done = Jatka
# Under the first-run sheet's title: what a choice does.
profile-intro = Jokainen valinta lisää omat työkalunsa: tilat, komennot, työkalurivin painikkeet ja Asetusten sivut. Jokainen tiedosto avautuu valinnasta riippumatta.
# Under each choice of the first-run sheet: what it is for.
# machine-drafted, needs native review
profile-about = { $capability ->
    [engineering] Tekniset piirustukset ja 3D-mallit.
    [medical] DICOM-tutkimukset ja lääketieteen tilavuudet, mittaukset ja fuusio.
    [science] Tähtitiede, mikroskopia ja tieteellinen data.
    [maps] Kartat, GPS-reitit ja korkeusmallit.
   *[general] Valokuvat ja arkiset kuvat. Aina käytössä.
}
# Above Continue on the first-run sheet.
profile-later = Voit muuttaa molempia milloin tahansa: profiilia kohdassa Asetukset › Profiili ja aloitusta kohdassa Asetukset › Yleiset.
# The toolbar chip shown while a medical (or science, engineering) image is on screen.
# machine-drafted, needs native review
profile-tools = { $capability ->
    [medical] Lääketieteen työkalut
    [science] Tieteen työkalut
    [maps] Karttatyökalut
    [engineering] Tekniikan työkalut
   *[general] Yleiset työkalut
}
# Why a command of a profile that is off is greyed out in the palette.
# machine-drafted, needs native review
profile-tools-off = { $capability ->
    [medical] Lääketieteen työkalut ovat pois päältä
    [science] Tieteen työkalut ovat pois päältä
    [maps] Karttatyökalut ovat pois päältä
    [engineering] Tekniikan työkalut ovat pois päältä
   *[general] Yleiset työkalut ovat pois päältä
}
# Toast after the chip turned the tools on; its button keeps them on.
# machine-drafted, needs native review
profile-tools-on = { $capability ->
    [medical] Lääketieteen työkalut ovat käytössä tämän istunnon ajan
    [science] Tieteen työkalut ovat käytössä tämän istunnon ajan
    [maps] Karttatyökalut ovat käytössä tämän istunnon ajan
    [engineering] Tekniikan työkalut ovat käytössä tämän istunnon ajan
   *[general] Yleiset työkalut ovat käytössä tämän istunnon ajan
}
profile-always = Aina
# Palette entries; $tools is a profile-tools label.
profile-turn-on = { $always ->
    [yes] Ota { $tools } aina käyttöön
   *[no] Ota { $tools } käyttöön tämän istunnon ajaksi
}
