# Specular — Profiles: what Specular is used for (the first-run question, Settings → Profile,
# the toolbar chip that turns a profile's tools on). Français.

settings-section-profile = Profil
settings-profile-capabilities = J’utilise { -app-name } pour
settings-profile-capabilities-help = Chaque choix ajoute ses outils : modes, commandes, boutons de la barre d’outils et pages des Paramètres. Général est toujours actif, et chaque format s’ouvre quel que soit le choix.
settings-profile-capabilities-general = Général
settings-profile-capabilities-engineering = Ingénierie
settings-profile-capabilities-medical = Médical
settings-profile-capabilities-science = Science
# machine-drafted, needs native review
settings-profile-capabilities-maps = Cartes (SIG)
settings-profile-shown = Déjà proposé
settings-profile-shown-help = La question du premier lancement et les propositions « Toujours » déjà affichées, pour ne plus les montrer.
# The first-run sheet's title; the choices are the settings-profile-capabilities-* labels.
profile-question = Pour quoi utiliserez-vous { -app-name } ?
profile-done = Continuer
# Under the first-run sheet's title: what a choice does.
profile-intro = Chaque choix ajoute ses propres outils : modes, commandes, boutons de la barre d’outils et pages des Paramètres. Chaque fichier s’ouvre quel que soit votre choix.
# Under each choice of the first-run sheet: what it is for.
# machine-drafted, needs native review
profile-about = { $capability ->
    [engineering] Dessins techniques et modèles 3D.
    [medical] Études DICOM et volumes médicaux, avec mesures et fusion.
    [science] Astronomie, microscopie et données scientifiques.
    [maps] Cartes, traces GPS et modèles d’élévation.
   *[general] Photos et images du quotidien. Toujours actif.
}
# Above Continue on the first-run sheet.
profile-later = Vous pouvez modifier les deux à tout moment : le profil dans Paramètres › Profil, le démarrage dans Paramètres › Général.
# The toolbar chip shown while a medical (or science, engineering) image is on screen.
# machine-drafted, needs native review
profile-tools = { $capability ->
    [medical] Outils médicaux
    [science] Outils scientifiques
    [maps] Outils cartographiques
    [engineering] Outils d’ingénierie
   *[general] Outils généraux
}
# Why a command of a profile that is off is greyed out in the palette.
# machine-drafted, needs native review
profile-tools-off = { $capability ->
    [medical] Les outils médicaux sont désactivés
    [science] Les outils scientifiques sont désactivés
    [maps] Les outils cartographiques sont désactivés
    [engineering] Les outils d’ingénierie sont désactivés
   *[general] Les outils généraux sont désactivés
}
# Toast after the chip turned the tools on; its button keeps them on.
# machine-drafted, needs native review
profile-tools-on = { $capability ->
    [medical] Les outils médicaux sont activés pour cette session
    [science] Les outils scientifiques sont activés pour cette session
    [maps] Les outils cartographiques sont activés pour cette session
    [engineering] Les outils d’ingénierie sont activés pour cette session
   *[general] Les outils généraux sont activés pour cette session
}
profile-always = Toujours
# Palette entries; $tools is a profile-tools label.
profile-turn-on = { $always ->
    [yes] Toujours activer { $tools }
   *[no] Activer { $tools } pour cette session
}
