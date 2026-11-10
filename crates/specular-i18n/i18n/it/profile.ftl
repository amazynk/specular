# Specular — Profiles: what Specular is used for (the first-run question, Settings → Profile,
# the toolbar chip that turns a profile's tools on). Italiano.

settings-section-profile = Profilo
settings-profile-capabilities = Per cosa uso { -app-name }
settings-profile-capabilities-help = Ogni scelta aggiunge i suoi strumenti: modalità, comandi, pulsanti della barra degli strumenti e pagine delle Impostazioni. Generale è sempre attivo e ogni formato si apre qualunque sia la scelta.
settings-profile-capabilities-general = Generale
settings-profile-capabilities-engineering = Ingegneria
settings-profile-capabilities-medical = Medicina
settings-profile-capabilities-science = Scienza
# machine-drafted, needs native review
settings-profile-capabilities-maps = Mappe (GIS)
settings-profile-shown = Già proposto
settings-profile-shown-help = La domanda al primo avvio e le proposte «Sempre» già mostrate, così non vengono riproposte.
# The first-run sheet's title; the choices are the settings-profile-capabilities-* labels.
profile-question = Per cosa userai { -app-name }?
profile-done = Continua
# Under the first-run sheet's title: what a choice does.
profile-intro = Ogni scelta aggiunge i propri strumenti: modalità, comandi, pulsanti della barra degli strumenti e pagine delle Impostazioni. Ogni file si apre qualunque sia la scelta.
# Under each choice of the first-run sheet: what it is for.
# machine-drafted, needs native review
profile-about = { $capability ->
    [engineering] Disegni tecnici e modelli 3D.
    [medical] Studi DICOM e volumi medici, con misure e fusione.
    [science] Astronomia, microscopia e dati scientifici.
    [maps] Mappe, tracce GPS e modelli di elevazione.
   *[general] Foto e immagini di tutti i giorni. Sempre attivo.
}
# Above Continue on the first-run sheet.
profile-later = Puoi cambiarli entrambi in qualsiasi momento: il profilo in Impostazioni › Profilo, l’avvio in Impostazioni › Generali.
# The toolbar chip shown while a medical (or science, engineering) image is on screen.
# machine-drafted, needs native review
profile-tools = { $capability ->
    [medical] Strumenti medici
    [science] Strumenti scientifici
    [maps] Strumenti per mappe
    [engineering] Strumenti di ingegneria
   *[general] Strumenti generali
}
# Why a command of a profile that is off is greyed out in the palette.
# machine-drafted, needs native review
profile-tools-off = { $capability ->
    [medical] Gli strumenti medici sono disattivati
    [science] Gli strumenti scientifici sono disattivati
    [maps] Gli strumenti per mappe sono disattivati
    [engineering] Gli strumenti di ingegneria sono disattivati
   *[general] Gli strumenti generali sono disattivati
}
# Toast after the chip turned the tools on; its button keeps them on.
# machine-drafted, needs native review
profile-tools-on = { $capability ->
    [medical] Gli strumenti medici sono attivi per questa sessione
    [science] Gli strumenti scientifici sono attivi per questa sessione
    [maps] Gli strumenti per mappe sono attivi per questa sessione
    [engineering] Gli strumenti di ingegneria sono attivi per questa sessione
   *[general] Gli strumenti generali sono attivi per questa sessione
}
profile-always = Sempre
# Palette entries; $tools is a profile-tools label.
profile-turn-on = { $always ->
    [yes] Attiva sempre { $tools }
   *[no] Attiva { $tools } per questa sessione
}
