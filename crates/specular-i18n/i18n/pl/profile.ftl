# Specular — Profiles: what Specular is used for (the first-run question, Settings → Profile,
# the toolbar chip that turns a profile's tools on). Polski.

settings-section-profile = Profil
settings-profile-capabilities = Do czego używam programu { -app-name }
settings-profile-capabilities-help = Każdy wybór dodaje swoje narzędzia: tryby, polecenia, przyciski paska narzędzi i strony Ustawień. Ogólne jest zawsze włączone, a każdy format otwiera się niezależnie od wyboru.
settings-profile-capabilities-general = Ogólne
settings-profile-capabilities-engineering = Inżynieria
settings-profile-capabilities-medical = Medycyna
settings-profile-capabilities-science = Nauka
# machine-drafted, needs native review
settings-profile-capabilities-maps = Mapy (GIS)
settings-profile-shown = Już zaproponowane
settings-profile-shown-help = Pytanie przy pierwszym uruchomieniu i już pokazane propozycje „Zawsze”, aby nie pojawiały się ponownie.
# The first-run sheet's title; the choices are the settings-profile-capabilities-* labels.
profile-question = Do czego będziesz używać programu { -app-name }?
profile-done = Dalej
# Under the first-run sheet's title: what a choice does.
profile-intro = Każdy wybór dodaje własne narzędzia: tryby, polecenia, przyciski paska narzędzi i strony Ustawień. Każdy plik otwiera się niezależnie od wyboru.
# Under each choice of the first-run sheet: what it is for.
# machine-drafted, needs native review ([maps] variant)
profile-about = { $capability ->
    [engineering] Rysunki techniczne i modele 3D.
    [medical] Badania DICOM i wolumeny medyczne, z pomiarami i fuzją.
    [science] Astronomia, mikroskopia i dane naukowe.
    [maps] Mapy, ślady GPS i modele wysokości.
   *[general] Zdjęcia i codzienne obrazy. Zawsze włączone.
}
# Above Continue on the first-run sheet.
profile-later = Oba możesz zmienić w dowolnej chwili: profil w Ustawienia › Profil, start w Ustawienia › Ogólne.
# The toolbar chip shown while a medical (or science, engineering) image is on screen.
# machine-drafted, needs native review ([maps] variant)
profile-tools = { $capability ->
    [medical] Narzędzia medyczne
    [science] Narzędzia naukowe
    [maps] Narzędzia map
    [engineering] Narzędzia inżynierskie
   *[general] Narzędzia ogólne
}
# Why a command of a profile that is off is greyed out in the palette.
# machine-drafted, needs native review ([maps] variant)
profile-tools-off = { $capability ->
    [medical] Narzędzia medyczne są wyłączone
    [science] Narzędzia naukowe są wyłączone
    [maps] Narzędzia map są wyłączone
    [engineering] Narzędzia inżynierskie są wyłączone
   *[general] Narzędzia ogólne są wyłączone
}
# Toast after the chip turned the tools on; its button keeps them on.
# machine-drafted, needs native review ([maps] variant)
profile-tools-on = { $capability ->
    [medical] Narzędzia medyczne są włączone na tę sesję
    [science] Narzędzia naukowe są włączone na tę sesję
    [maps] Narzędzia map są włączone na tę sesję
    [engineering] Narzędzia inżynierskie są włączone na tę sesję
   *[general] Narzędzia ogólne są włączone na tę sesję
}
profile-always = Zawsze
# Palette entries; $tools is a profile-tools label.
profile-turn-on = { $always ->
    [yes] Zawsze włączaj { $tools }
   *[no] Włącz { $tools } na tę sesję
}
