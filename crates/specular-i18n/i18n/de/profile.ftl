# Specular — Profiles: what Specular is used for (the first-run question, Settings → Profile,
# the toolbar chip that turns a profile's tools on). Deutsch.

settings-section-profile = Profil
settings-profile-capabilities = Wofür ich { -app-name } nutze
settings-profile-capabilities-help = Jede Auswahl fügt ihre Werkzeuge hinzu: Modi, Befehle, Symbolleistenschaltflächen und Einstellungsseiten. Allgemein ist immer aktiv, und jedes Format wird unabhängig von der Auswahl geöffnet.
settings-profile-capabilities-general = Allgemein
settings-profile-capabilities-engineering = Technik
settings-profile-capabilities-medical = Medizin
settings-profile-capabilities-science = Wissenschaft
# machine-drafted, needs native review
settings-profile-capabilities-maps = Karten (GIS)
settings-profile-shown = Bereits angeboten
settings-profile-shown-help = Die Frage beim ersten Start und die bereits gezeigten „Immer“-Angebote, damit sie nicht erneut erscheinen.
# The first-run sheet's title; the choices are the settings-profile-capabilities-* labels.
profile-question = Wofür werden Sie { -app-name } nutzen?
profile-done = Weiter
# Under the first-run sheet's title: what a choice does.
profile-intro = Jede Auswahl fügt eigene Werkzeuge hinzu: Modi, Befehle, Symbolleistenschaltflächen und Einstellungsseiten. Jede Datei wird unabhängig von der Auswahl geöffnet.
# Under each choice of the first-run sheet: what it is for.
# machine-drafted, needs native review
profile-about = { $capability ->
    [engineering] Technische Zeichnungen und 3D-Modelle.
    [medical] DICOM-Studien und medizinische Volumen, mit Messungen und Fusion.
    [science] Astronomie, Mikroskopie und wissenschaftliche Daten.
    [maps] Karten, GPS-Tracks und Höhenmodelle.
   *[general] Fotos und alltägliche Bilder. Immer aktiv.
}
# Above Continue on the first-run sheet.
profile-later = Sie können beides jederzeit ändern: das Profil unter Einstellungen › Profil, den Start unter Einstellungen › Allgemein.
# The toolbar chip shown while a medical (or science, engineering) image is on screen.
# machine-drafted, needs native review
profile-tools = { $capability ->
    [medical] Medizin-Werkzeuge
    [science] Wissenschafts-Werkzeuge
    [maps] Karten-Werkzeuge
    [engineering] Technik-Werkzeuge
   *[general] Allgemeine Werkzeuge
}
# Why a command of a profile that is off is greyed out in the palette.
# machine-drafted, needs native review
profile-tools-off = { $capability ->
    [medical] Medizin-Werkzeuge sind aus
    [science] Wissenschafts-Werkzeuge sind aus
    [maps] Karten-Werkzeuge sind aus
    [engineering] Technik-Werkzeuge sind aus
   *[general] Allgemeine Werkzeuge sind aus
}
# Toast after the chip turned the tools on; its button keeps them on.
# machine-drafted, needs native review
profile-tools-on = { $capability ->
    [medical] Medizin-Werkzeuge sind für diese Sitzung an
    [science] Wissenschafts-Werkzeuge sind für diese Sitzung an
    [maps] Karten-Werkzeuge sind für diese Sitzung an
    [engineering] Technik-Werkzeuge sind für diese Sitzung an
   *[general] Allgemeine Werkzeuge sind für diese Sitzung an
}
profile-always = Immer
# Palette entries; $tools is a profile-tools label.
profile-turn-on = { $always ->
    [yes] { $tools } immer einschalten
   *[no] { $tools } für diese Sitzung einschalten
}
