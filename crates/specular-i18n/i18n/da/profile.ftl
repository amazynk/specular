# Specular — Profiles: what Specular is used for (the first-run question, Settings → Profile,
# the toolbar chip that turns a profile's tools on). Dansk.

settings-section-profile = Profil
settings-profile-capabilities = Hvad jeg bruger { -app-name } til
settings-profile-capabilities-help = Hvert valg tilføjer sine værktøjer: tilstande, kommandoer, knapper på værktøjslinjen og sider i Indstillinger. Generelt er altid slået til, og alle formater åbnes uanset valget.
settings-profile-capabilities-general = Generelt
settings-profile-capabilities-engineering = Teknik
settings-profile-capabilities-medical = Medicin
settings-profile-capabilities-science = Videnskab
# machine-drafted, needs native review
settings-profile-capabilities-maps = Kort (GIS)
settings-profile-shown = Allerede tilbudt
settings-profile-shown-help = Spørgsmålet ved første start og de ”Altid”-tilbud, der allerede er vist, så de ikke vises igen.
# The first-run sheet's title; the choices are the settings-profile-capabilities-* labels.
profile-question = Hvad vil du bruge { -app-name } til?
profile-done = Fortsæt
# Under the first-run sheet's title: what a choice does.
profile-intro = Hvert valg tilføjer sine egne værktøjer: tilstande, kommandoer, knapper på værktøjslinjen og sider i Indstillinger. Alle filer åbnes uanset dit valg.
# Under each choice of the first-run sheet: what it is for.
# machine-drafted, needs native review
profile-about = { $capability ->
    [engineering] Tekniske tegninger og 3D-modeller.
    [medical] DICOM-undersøgelser og medicinske volumener med målinger og fusion.
    [science] Astronomi, mikroskopi og videnskabelige data.
    [maps] Kort, GPS-spor og højdemodeller.
   *[general] Fotos og hverdagsbilleder. Altid slået til.
}
# Above Continue on the first-run sheet.
profile-later = Du kan til enhver tid ændre begge: profilen under Indstillinger › Profil, starten under Indstillinger › Generelt.
# The toolbar chip shown while a medical (or science, engineering) image is on screen.
# machine-drafted, needs native review
profile-tools = { $capability ->
    [medical] Medicinske værktøjer
    [science] Videnskabelige værktøjer
    [maps] Kortværktøjer
    [engineering] Tekniske værktøjer
   *[general] Generelle værktøjer
}
# Why a command of a profile that is off is greyed out in the palette.
# machine-drafted, needs native review
profile-tools-off = { $capability ->
    [medical] Medicinske værktøjer er slået fra
    [science] Videnskabelige værktøjer er slået fra
    [maps] Kortværktøjer er slået fra
    [engineering] Tekniske værktøjer er slået fra
   *[general] Generelle værktøjer er slået fra
}
# Toast after the chip turned the tools on; its button keeps them on.
# machine-drafted, needs native review
profile-tools-on = { $capability ->
    [medical] Medicinske værktøjer er slået til i denne session
    [science] Videnskabelige værktøjer er slået til i denne session
    [maps] Kortværktøjer er slået til i denne session
    [engineering] Tekniske værktøjer er slået til i denne session
   *[general] Generelle værktøjer er slået til i denne session
}
profile-always = Altid
# Palette entries; $tools is a profile-tools label.
profile-turn-on = { $always ->
    [yes] Slå { $tools } til altid
   *[no] Slå { $tools } til i denne session
}
