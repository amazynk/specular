# Specular — Profiles: what Specular is used for (the first-run question, Settings → Profile,
# the toolbar chip that turns a profile's tools on). English.

settings-section-profile = Profile
settings-profile-capabilities = What I use { -app-name } for
settings-profile-capabilities-help = Each choice adds its tools: modes, commands, toolbar buttons and Settings pages. General is always on, and every format opens whatever is chosen.
settings-profile-capabilities-general = General
settings-profile-capabilities-engineering = Engineering
settings-profile-capabilities-medical = Medical
settings-profile-capabilities-science = Science
settings-profile-capabilities-maps = Maps (GIS)
settings-profile-shown = Already offered
settings-profile-shown-help = The first-run question and the “Always” offers already shown, so they are not shown again.
# The first-run sheet's title; the choices are the settings-profile-capabilities-* labels.
profile-question = What will you use { -app-name } for?
profile-done = Continue
# Under the first-run sheet's title: what a choice does.
profile-intro = Each choice adds its own tools: modes, commands, toolbar buttons and Settings pages. Every file opens whatever you choose.
# Under each choice of the first-run sheet: what it is for.
profile-about = { $capability ->
    [engineering] Technical drawings and 3D models.
    [medical] DICOM studies and medical volumes, with measurements and fusion.
    [science] Astronomy, microscopy and scientific data.
    [maps] Maps, GPS tracks and elevation models.
   *[general] Photos and everyday images. Always on.
}
# Above Continue on the first-run sheet.
profile-later = You can change both at any time: the profile in Settings › Profile, the start in Settings › General.
# The toolbar chip shown while a medical (or science, engineering) image is on screen.
profile-tools = { $capability ->
    [medical] Medical tools
    [science] Science tools
    [maps] Maps tools
    [engineering] Engineering tools
   *[general] General tools
}
# Why a command of a profile that is off is greyed out in the palette.
profile-tools-off = { $capability ->
    [medical] Medical tools are off
    [science] Science tools are off
    [maps] Maps tools are off
    [engineering] Engineering tools are off
   *[general] General tools are off
}
# Toast after the chip turned the tools on; its button keeps them on.
profile-tools-on = { $capability ->
    [medical] Medical tools are on for this session
    [science] Science tools are on for this session
    [maps] Maps tools are on for this session
    [engineering] Engineering tools are on for this session
   *[general] General tools are on for this session
}
profile-always = Always
# Palette entries; $tools is a profile-tools label.
profile-turn-on = { $always ->
    [yes] Turn on { $tools } always
   *[no] Turn on { $tools } for this session
}
