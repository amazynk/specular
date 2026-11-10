# Specular — Profiles: what Specular is used for (the first-run question, Settings → Profile,
# the toolbar chip that turns a profile's tools on). Čeština.

settings-section-profile = Profil
settings-profile-capabilities = K čemu používám { -app-name }
settings-profile-capabilities-help = Každá volba přidá své nástroje: režimy, příkazy, tlačítka panelu nástrojů a stránky Nastavení. Obecné je vždy zapnuté a každý formát se otevře bez ohledu na volbu.
settings-profile-capabilities-general = Obecné
settings-profile-capabilities-engineering = Technika
settings-profile-capabilities-medical = Medicína
settings-profile-capabilities-science = Věda
# machine-drafted, needs native review
settings-profile-capabilities-maps = Mapy (GIS)
settings-profile-shown = Již nabídnuto
settings-profile-shown-help = Otázka při prvním spuštění a již zobrazené nabídky „Vždy“, aby se znovu nezobrazovaly.
# The first-run sheet's title; the choices are the settings-profile-capabilities-* labels.
profile-question = K čemu budete { -app-name } používat?
profile-done = Pokračovat
# Under the first-run sheet's title: what a choice does.
profile-intro = Každá volba přidá vlastní nástroje: režimy, příkazy, tlačítka panelu nástrojů a stránky Nastavení. Každý soubor se otevře bez ohledu na volbu.
# Under each choice of the first-run sheet: what it is for.
# machine-drafted, needs native review
profile-about = { $capability ->
    [engineering] Technické výkresy a 3D modely.
    [medical] Studie DICOM a lékařské objemy, s měřením a fúzí.
    [science] Astronomie, mikroskopie a vědecká data.
    [maps] Mapy, trasy GPS a výškové modely.
   *[general] Fotky a běžné obrázky. Vždy zapnuto.
}
# Above Continue on the first-run sheet.
profile-later = Obojí můžete kdykoli změnit: profil v Nastavení › Profil, úvodní obrazovku v Nastavení › Obecné.
# The toolbar chip shown while a medical (or science, engineering) image is on screen.
# machine-drafted, needs native review
profile-tools = { $capability ->
    [medical] Lékařské nástroje
    [science] Vědecké nástroje
    [maps] Mapové nástroje
    [engineering] Technické nástroje
   *[general] Obecné nástroje
}
# Why a command of a profile that is off is greyed out in the palette.
# machine-drafted, needs native review
profile-tools-off = { $capability ->
    [medical] Lékařské nástroje jsou vypnuté
    [science] Vědecké nástroje jsou vypnuté
    [maps] Mapové nástroje jsou vypnuté
    [engineering] Technické nástroje jsou vypnuté
   *[general] Obecné nástroje jsou vypnuté
}
# Toast after the chip turned the tools on; its button keeps them on.
# machine-drafted, needs native review
profile-tools-on = { $capability ->
    [medical] Lékařské nástroje jsou zapnuté pro tuto relaci
    [science] Vědecké nástroje jsou zapnuté pro tuto relaci
    [maps] Mapové nástroje jsou zapnuté pro tuto relaci
    [engineering] Technické nástroje jsou zapnuté pro tuto relaci
   *[general] Obecné nástroje jsou zapnuté pro tuto relaci
}
profile-always = Vždy
# Palette entries; $tools is a profile-tools label.
profile-turn-on = { $always ->
    [yes] Vždy zapnout { $tools }
   *[no] Zapnout { $tools } pro tuto relaci
}
