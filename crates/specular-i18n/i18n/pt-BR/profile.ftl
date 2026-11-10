# Specular — Profiles: what Specular is used for (the first-run question, Settings → Profile,
# the toolbar chip that turns a profile's tools on). Português (Brasil).

settings-section-profile = Perfil
settings-profile-capabilities = Para que uso o { -app-name }
settings-profile-capabilities-help = Cada escolha adiciona suas ferramentas: modos, comandos, botões da barra de ferramentas e páginas de Configurações. Geral está sempre ativo, e todo formato abre independentemente da escolha.
settings-profile-capabilities-general = Geral
settings-profile-capabilities-engineering = Engenharia
settings-profile-capabilities-medical = Medicina
settings-profile-capabilities-science = Ciência
# machine-drafted, needs native review
settings-profile-capabilities-maps = Mapas (SIG)
settings-profile-shown = Já oferecido
settings-profile-shown-help = A pergunta da primeira execução e as ofertas “Sempre” já mostradas, para que não apareçam de novo.
# The first-run sheet's title; the choices are the settings-profile-capabilities-* labels.
profile-question = Para que você vai usar o { -app-name }?
profile-done = Continuar
# Under the first-run sheet's title: what a choice does.
profile-intro = Cada escolha adiciona suas próprias ferramentas: modos, comandos, botões da barra de ferramentas e páginas de Configurações. Todo arquivo abre independentemente da escolha.
# Under each choice of the first-run sheet: what it is for.
# machine-drafted, needs native review ([maps] variant)
profile-about = { $capability ->
    [engineering] Desenhos técnicos e modelos 3D.
    [medical] Estudos DICOM e volumes médicos, com medições e fusão.
    [science] Astronomia, microscopia e dados científicos.
    [maps] Mapas, trilhas GPS e modelos de elevação.
   *[general] Fotos e imagens do dia a dia. Sempre ativo.
}
# Above Continue on the first-run sheet.
profile-later = Você pode mudar os dois a qualquer momento: o perfil em Configurações › Perfil e o início em Configurações › Geral.
# The toolbar chip shown while a medical (or science, engineering) image is on screen.
# machine-drafted, needs native review ([maps] variant)
profile-tools = { $capability ->
    [medical] Ferramentas médicas
    [science] Ferramentas científicas
    [maps] Ferramentas de mapas
    [engineering] Ferramentas de engenharia
   *[general] Ferramentas gerais
}
# Why a command of a profile that is off is greyed out in the palette.
# machine-drafted, needs native review ([maps] variant)
profile-tools-off = { $capability ->
    [medical] As ferramentas médicas estão desativadas
    [science] As ferramentas científicas estão desativadas
    [maps] As ferramentas de mapas estão desativadas
    [engineering] As ferramentas de engenharia estão desativadas
   *[general] As ferramentas gerais estão desativadas
}
# Toast after the chip turned the tools on; its button keeps them on.
# machine-drafted, needs native review ([maps] variant)
profile-tools-on = { $capability ->
    [medical] As ferramentas médicas estão ativadas nesta sessão
    [science] As ferramentas científicas estão ativadas nesta sessão
    [maps] As ferramentas de mapas estão ativadas nesta sessão
    [engineering] As ferramentas de engenharia estão ativadas nesta sessão
   *[general] As ferramentas gerais estão ativadas nesta sessão
}
profile-always = Sempre
# Palette entries; $tools is a profile-tools label.
profile-turn-on = { $always ->
    [yes] Ativar { $tools } sempre
   *[no] Ativar { $tools } nesta sessão
}
