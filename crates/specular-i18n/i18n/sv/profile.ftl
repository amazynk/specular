# Specular — Profiles: what Specular is used for (the first-run question, Settings → Profile,
# the toolbar chip that turns a profile's tools on). Svenska.

settings-section-profile = Profil
settings-profile-capabilities = Vad jag använder { -app-name } till
settings-profile-capabilities-help = Varje val lägger till sina verktyg: lägen, kommandon, knappar i verktygsfältet och sidor i Inställningar. Allmänt är alltid på, och alla format öppnas oavsett val.
settings-profile-capabilities-general = Allmänt
settings-profile-capabilities-engineering = Teknik
settings-profile-capabilities-medical = Medicin
settings-profile-capabilities-science = Vetenskap
# machine-drafted, needs native review
settings-profile-capabilities-maps = Kartor (GIS)
settings-profile-shown = Redan erbjudet
settings-profile-shown-help = Frågan vid första start och de ”Alltid”-erbjudanden som redan visats, så att de inte visas igen.
# The first-run sheet's title; the choices are the settings-profile-capabilities-* labels.
profile-question = Vad kommer du att använda { -app-name } till?
profile-done = Fortsätt
# Under the first-run sheet's title: what a choice does.
profile-intro = Varje val lägger till egna verktyg: lägen, kommandon, knappar i verktygsfältet och sidor i Inställningar. Alla filer öppnas oavsett val.
# Under each choice of the first-run sheet: what it is for.
# machine-drafted, needs native review ([maps] variant)
profile-about = { $capability ->
    [engineering] Tekniska ritningar och 3D-modeller.
    [medical] DICOM-undersökningar och medicinska volymer, med mätningar och fusion.
    [science] Astronomi, mikroskopi och vetenskapliga data.
    [maps] Kartor, GPS-spår och höjdmodeller.
   *[general] Foton och vardagsbilder. Alltid på.
}
# Above Continue on the first-run sheet.
profile-later = Du kan ändra båda när som helst: profilen under Inställningar › Profil, starten under Inställningar › Allmänt.
# The toolbar chip shown while a medical (or science, engineering) image is on screen.
# machine-drafted, needs native review ([maps] variant)
profile-tools = { $capability ->
    [medical] Medicinska verktyg
    [science] Vetenskapliga verktyg
    [maps] Kartverktyg
    [engineering] Tekniska verktyg
   *[general] Allmänna verktyg
}
# Why a command of a profile that is off is greyed out in the palette.
# machine-drafted, needs native review ([maps] variant)
profile-tools-off = { $capability ->
    [medical] Medicinska verktyg är av
    [science] Vetenskapliga verktyg är av
    [maps] Kartverktyg är av
    [engineering] Tekniska verktyg är av
   *[general] Allmänna verktyg är av
}
# Toast after the chip turned the tools on; its button keeps them on.
# machine-drafted, needs native review ([maps] variant)
profile-tools-on = { $capability ->
    [medical] Medicinska verktyg är på för den här sessionen
    [science] Vetenskapliga verktyg är på för den här sessionen
    [maps] Kartverktyg är på för den här sessionen
    [engineering] Tekniska verktyg är på för den här sessionen
   *[general] Allmänna verktyg är på för den här sessionen
}
profile-always = Alltid
# Palette entries; $tools is a profile-tools label.
profile-turn-on = { $always ->
    [yes] Slå på { $tools } alltid
   *[no] Slå på { $tools } för den här sessionen
}
