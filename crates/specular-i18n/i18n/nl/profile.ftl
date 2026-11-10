# Specular — Profiles: what Specular is used for (the first-run question, Settings → Profile,
# the toolbar chip that turns a profile's tools on). Nederlands.

settings-section-profile = Profiel
settings-profile-capabilities = Waarvoor ik { -app-name } gebruik
settings-profile-capabilities-help = Elke keuze voegt eigen hulpmiddelen toe: modi, opdrachten, werkbalkknoppen en pagina’s in Instellingen. Algemeen staat altijd aan, en elk formaat opent ongeacht de keuze.
settings-profile-capabilities-general = Algemeen
settings-profile-capabilities-engineering = Techniek
settings-profile-capabilities-medical = Medisch
settings-profile-capabilities-science = Wetenschap
# machine-drafted, needs native review
settings-profile-capabilities-maps = Kaarten (GIS)
settings-profile-shown = Al aangeboden
settings-profile-shown-help = De vraag bij de eerste start en de al getoonde ‘Altijd’-aanbiedingen, zodat ze niet opnieuw verschijnen.
# The first-run sheet's title; the choices are the settings-profile-capabilities-* labels.
profile-question = Waarvoor ga je { -app-name } gebruiken?
profile-done = Doorgaan
# Under the first-run sheet's title: what a choice does.
profile-intro = Elke keuze voegt eigen hulpmiddelen toe: modi, opdrachten, werkbalkknoppen en pagina’s in Instellingen. Elk bestand opent ongeacht je keuze.
# Under each choice of the first-run sheet: what it is for.
# machine-drafted, needs native review ([maps] variant)
profile-about = { $capability ->
    [engineering] Technische tekeningen en 3D-modellen.
    [medical] DICOM-onderzoeken en medische volumes, met metingen en fusie.
    [science] Astronomie, microscopie en wetenschappelijke data.
    [maps] Kaarten, gps-tracks en hoogtemodellen.
   *[general] Foto’s en alledaagse afbeeldingen. Staat altijd aan.
}
# Above Continue on the first-run sheet.
profile-later = Je kunt beide altijd wijzigen: het profiel via Instellingen › Profiel, het begin via Instellingen › Algemeen.
# The toolbar chip shown while a medical (or science, engineering) image is on screen.
# machine-drafted, needs native review ([maps] variant)
profile-tools = { $capability ->
    [medical] Medische hulpmiddelen
    [science] Wetenschappelijke hulpmiddelen
    [maps] Kaarthulpmiddelen
    [engineering] Technische hulpmiddelen
   *[general] Algemene hulpmiddelen
}
# Why a command of a profile that is off is greyed out in the palette.
# machine-drafted, needs native review ([maps] variant)
profile-tools-off = { $capability ->
    [medical] Medische hulpmiddelen staan uit
    [science] Wetenschappelijke hulpmiddelen staan uit
    [maps] Kaarthulpmiddelen staan uit
    [engineering] Technische hulpmiddelen staan uit
   *[general] Algemene hulpmiddelen staan uit
}
# Toast after the chip turned the tools on; its button keeps them on.
# machine-drafted, needs native review ([maps] variant)
profile-tools-on = { $capability ->
    [medical] Medische hulpmiddelen staan aan voor deze sessie
    [science] Wetenschappelijke hulpmiddelen staan aan voor deze sessie
    [maps] Kaarthulpmiddelen staan aan voor deze sessie
    [engineering] Technische hulpmiddelen staan aan voor deze sessie
   *[general] Algemene hulpmiddelen staan aan voor deze sessie
}
profile-always = Altijd
# Palette entries; $tools is a profile-tools label.
profile-turn-on = { $always ->
    [yes] { $tools } altijd aanzetten
   *[no] { $tools } aanzetten voor deze sessie
}
