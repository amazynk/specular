# Specular — Profiles: what Specular is used for (the first-run question, Settings → Profile,
# the toolbar chip that turns a profile's tools on). Español.

settings-section-profile = Perfil
settings-profile-capabilities = Para qué uso { -app-name }
settings-profile-capabilities-help = Cada opción añade sus herramientas: modos, comandos, botones de la barra de herramientas y páginas de Configuración. General siempre está activo, y todos los formatos se abren sea cual sea la elección.
settings-profile-capabilities-general = General
settings-profile-capabilities-engineering = Ingeniería
settings-profile-capabilities-medical = Medicina
settings-profile-capabilities-science = Ciencia
# machine-drafted, needs native review
settings-profile-capabilities-maps = Mapas (SIG)
settings-profile-shown = Ya ofrecido
settings-profile-shown-help = La pregunta del primer inicio y las ofertas «Siempre» ya mostradas, para no volver a mostrarlas.
# The first-run sheet's title; the choices are the settings-profile-capabilities-* labels.
profile-question = ¿Para qué usará { -app-name }?
profile-done = Continuar
# Under the first-run sheet's title: what a choice does.
profile-intro = Cada opción añade sus propias herramientas: modos, comandos, botones de la barra de herramientas y páginas de Configuración. Todos los archivos se abren sea cual sea la elección.
# Under each choice of the first-run sheet: what it is for.
# machine-drafted, needs native review
profile-about = { $capability ->
    [engineering] Planos técnicos y modelos 3D.
    [medical] Estudios DICOM y volúmenes médicos, con mediciones y fusión.
    [science] Astronomía, microscopía y datos científicos.
    [maps] Mapas, tracks GPS y modelos de elevación.
   *[general] Fotos e imágenes cotidianas. Siempre activo.
}
# Above Continue on the first-run sheet.
profile-later = Puede cambiar ambos en cualquier momento: el perfil en Configuración › Perfil y el inicio en Configuración › General.
# The toolbar chip shown while a medical (or science, engineering) image is on screen.
# machine-drafted, needs native review
profile-tools = { $capability ->
    [medical] Herramientas médicas
    [science] Herramientas científicas
    [maps] Herramientas de mapas
    [engineering] Herramientas de ingeniería
   *[general] Herramientas generales
}
# Why a command of a profile that is off is greyed out in the palette.
# machine-drafted, needs native review
profile-tools-off = { $capability ->
    [medical] Las herramientas médicas están desactivadas
    [science] Las herramientas científicas están desactivadas
    [maps] Las herramientas de mapas están desactivadas
    [engineering] Las herramientas de ingeniería están desactivadas
   *[general] Las herramientas generales están desactivadas
}
# Toast after the chip turned the tools on; its button keeps them on.
# machine-drafted, needs native review
profile-tools-on = { $capability ->
    [medical] Las herramientas médicas están activadas en esta sesión
    [science] Las herramientas científicas están activadas en esta sesión
    [maps] Las herramientas de mapas están activadas en esta sesión
    [engineering] Las herramientas de ingeniería están activadas en esta sesión
   *[general] Las herramientas generales están activadas en esta sesión
}
profile-always = Siempre
# Palette entries; $tools is a profile-tools label.
profile-turn-on = { $always ->
    [yes] Activar { $tools } siempre
   *[no] Activar { $tools } en esta sesión
}
