# Specular — Profiles: what Specular is used for (the first-run question, Settings → Profile,
# the toolbar chip that turns a profile's tools on). 日本語.

settings-section-profile = プロファイル
settings-profile-capabilities = { -app-name } の用途
settings-profile-capabilities-help = 選んだ項目ごとにツールが追加されます：モード、コマンド、ツールバーのボタン、設定のページ。一般は常にオンで、どの形式も選択にかかわらず開けます。
settings-profile-capabilities-general = 一般
settings-profile-capabilities-engineering = エンジニアリング
settings-profile-capabilities-medical = 医療
settings-profile-capabilities-science = 科学
# machine-drafted, needs native review
settings-profile-capabilities-maps = 地図 (GIS)
settings-profile-shown = 提示済み
settings-profile-shown-help = 初回起動時の質問と、すでに表示した「常に」の提案です。再表示されません。
# The first-run sheet's title; the choices are the settings-profile-capabilities-* labels.
profile-question = { -app-name } を何に使いますか？
profile-done = 続行
# Under the first-run sheet's title: what a choice does.
profile-intro = 選んだ項目ごとに専用のツールが追加されます：モード、コマンド、ツールバーのボタン、設定のページ。どのファイルも選択にかかわらず開けます。
# Under each choice of the first-run sheet: what it is for.
# machine-drafted, needs native review ([maps] variant)
profile-about = { $capability ->
    [engineering] 技術図面と 3D モデル。
    [medical] DICOM 検査と医用ボリューム（計測と融合に対応）。
    [science] 天文学、顕微鏡、科学データ。
    [maps] 地図、GPS トラック、標高モデル。
   *[general] 写真や日常の画像。常にオン。
}
# Above Continue on the first-run sheet.
profile-later = どちらもいつでも変更できます。プロファイルは「設定 › プロファイル」、起動時の表示は「設定 › 一般」で変更します。
# The toolbar chip shown while a medical (or science, engineering) image is on screen.
# machine-drafted, needs native review ([maps] variant)
profile-tools = { $capability ->
    [medical] 医療ツール
    [science] 科学ツール
    [maps] 地図ツール
    [engineering] エンジニアリングツール
   *[general] 一般ツール
}
# Why a command of a profile that is off is greyed out in the palette.
# machine-drafted, needs native review ([maps] variant)
profile-tools-off = { $capability ->
    [medical] 医療ツールはオフです
    [science] 科学ツールはオフです
    [maps] 地図ツールはオフです
    [engineering] エンジニアリングツールはオフです
   *[general] 一般ツールはオフです
}
# Toast after the chip turned the tools on; its button keeps them on.
# machine-drafted, needs native review ([maps] variant)
profile-tools-on = { $capability ->
    [medical] このセッションでは医療ツールがオンです
    [science] このセッションでは科学ツールがオンです
    [maps] このセッションでは地図ツールがオンです
    [engineering] このセッションではエンジニアリングツールがオンです
   *[general] このセッションでは一般ツールがオンです
}
profile-always = 常に
# Palette entries; $tools is a profile-tools label.
profile-turn-on = { $always ->
    [yes] { $tools }を常にオンにする
   *[no] このセッションで{ $tools }をオンにする
}
