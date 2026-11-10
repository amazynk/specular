# Specular — Profiles: what Specular is used for (the first-run question, Settings → Profile,
# the toolbar chip that turns a profile's tools on). 简体中文.

settings-section-profile = 使用场景
settings-profile-capabilities = 我用 { -app-name } 做什么
settings-profile-capabilities-help = 每个选项都会添加相应的工具：模式、命令、工具栏按钮和设置页面。通用始终开启，无论选择什么，所有格式都能打开。
settings-profile-capabilities-general = 通用
settings-profile-capabilities-engineering = 工程
settings-profile-capabilities-medical = 医学
settings-profile-capabilities-science = 科学
# machine-drafted, needs native review
settings-profile-capabilities-maps = 地图（GIS）
settings-profile-shown = 已提示过
settings-profile-shown-help = 首次启动时的问题和已显示过的“始终”提示，不会再次显示。
# The first-run sheet's title; the choices are the settings-profile-capabilities-* labels.
profile-question = 你将用 { -app-name } 做什么？
profile-done = 继续
# Under the first-run sheet's title: what a choice does.
profile-intro = 每个选项都会添加相应的工具：模式、命令、工具栏按钮和设置页面。无论选择什么，所有文件都能打开。
# Under each choice of the first-run sheet: what it is for.
# machine-drafted, needs native review ([maps] variant)
profile-about = { $capability ->
    [engineering] 工程图纸和 3D 模型。
    [medical] DICOM 检查和医学体数据，支持测量和融合。
    [science] 天文、显微和科学数据。
    [maps] 地图、GPS 轨迹和高程模型。
   *[general] 照片和日常图片。始终开启。
}
# Above Continue on the first-run sheet.
profile-later = 两者都可以随时更改：使用场景在“设置 › 使用场景”中，启动时的显示在“设置 › 通用”中。
# The toolbar chip shown while a medical (or science, engineering) image is on screen.
# machine-drafted, needs native review ([maps] variant)
profile-tools = { $capability ->
    [medical] 医学工具
    [science] 科学工具
    [maps] 地图工具
    [engineering] 工程工具
   *[general] 通用工具
}
# Why a command of a profile that is off is greyed out in the palette.
# machine-drafted, needs native review ([maps] variant)
profile-tools-off = { $capability ->
    [medical] 医学工具已关闭
    [science] 科学工具已关闭
    [maps] 地图工具已关闭
    [engineering] 工程工具已关闭
   *[general] 通用工具已关闭
}
# Toast after the chip turned the tools on; its button keeps them on.
# machine-drafted, needs native review ([maps] variant)
profile-tools-on = { $capability ->
    [medical] 医学工具已在本次会话中开启
    [science] 科学工具已在本次会话中开启
    [maps] 地图工具已在本次会话中开启
    [engineering] 工程工具已在本次会话中开启
   *[general] 通用工具已在本次会话中开启
}
profile-always = 始终
# Palette entries; $tools is a profile-tools label.
profile-turn-on = { $always ->
    [yes] 始终开启{ $tools }
   *[no] 在本次会话中开启{ $tools }
}
