# machine-drafted, needs native review
## Optional decoders: other vendors' free software that opens a few formats (Blackmagic RAW,
## DWG), used only when the user installed it. Specular never bundles or downloads it.
## $format is the format ("Blackmagic RAW", "DWG"), $decoder the decoder's own name
## ("Blackmagic RAW SDK", "ODA File Converter"), $vendor its maker ("Blackmagic Design",
## "Open Design Alliance"): names, left as they are.

## Settings › Optional decoders. The switches and paths are shown in the panel below
## "Look again", not as rows of their own.

settings-section-decoders = 可选解码器
settings-decoders-look-again = 重新查找
settings-decoders-look-again-help = 查找此后安装的解码器。
    每次窗口回到前台时，{ -app-name } 也会查找。
settings-decoders-braw = 使用 Blackmagic RAW SDK
settings-decoders-braw-help = 安装后，Blackmagic RAW 片段会用它打开。
settings-decoders-braw-path = Blackmagic RAW SDK
settings-decoders-braw-path-help = 留空则在其安装程序放置的位置查找。
settings-decoders-dwg = 使用 ODA File Converter
settings-decoders-dwg-help = 安装后，DWG 图纸会用它打开。
    图纸的副本在一个私有文件夹中转换，之后该文件夹会被删除。
settings-decoders-dwg-path = ODA File Converter 程序
settings-decoders-dwg-path-help = 留空则在其安装程序放置的位置查找。
settings-decoders-occt = 使用 OpenCASCADE
settings-decoders-occt-help = 安装后，STEP 和 IGES 模型会用它打开。
settings-decoders-occt-path = OpenCASCADE 库
settings-decoders-occt-path-help = 留空则在系统存放库的位置查找。
# machine-drafted, needs native review
settings-decoders-czi = 使用 libCZI
settings-decoders-czi-help = 安装后，Zeiss CZI 文件用它打开。
    它在 { -app-name } 的沙盒辅助进程中运行，无法访问你的其他文件。
settings-decoders-czi-path = libCZI 库文件夹
settings-decoders-czi-path-help = 留空则在系统存放库的位置查找。

## The panel: one block per decoder.

decoders-intro = 某些格式需要用其开发商的免费软件打开。
    只需安装一次：{ -app-name } 会找到并使用它，但从不下载它。
decoders-keywords = 可选 解码器 开发商 软件 编解码器 插件 需要解码器 optional decoders vendor SDK codec plugin Blackmagic RAW BRAW RED R3D DWG ODA File Converter OpenCASCADE STEP IGES Zeiss CZI libCZI needs decoder
# A decoder's row: "Blackmagic RAW (.braw)". The tick box before it switches it on and off.
decoders-row = { $format } ({ $extensions })
# The row's value.
decoders-state-found = 已找到
decoders-state-off = 关
decoders-state-not-found = 未安装
decoders-state-broken = 无法使用
decoders-state-refused = 不可行
decoders-state-planned = 暂不可用
decoders-state-checking = 正在查找…
# The row's lines.
decoders-uses = 用 { $vendor } 的 { $decoder } 打开。
decoders-not-found = 它是免费的。安装后按“重新查找”。
decoders-off = 已关闭：这些文件不会打开。
decoders-version = 版本 { $version }。
decoders-r3d-why = RED 的许可只允许按 RED 自己的条款、用其 SDK 构建的程序读取 R3D 文件。
    { -app-name } 无法接受这些条款，因此无法打开 R3D 文件。
decoders-ecw-why = Hexagon 的许可只允许随附 ECW SDK 的程序使用它，不允许使用你自行安装的副本。
    因此无法打开 ECW 文件。由 ECW SDK 生成的 JPEG 2000 文件可以打开。
decoders-mrsid-why = MrSID SDK 的许可只允许随附该 SDK 的程序使用它，不允许使用你自行安装的副本。
    因此无法打开 MrSID 文件。
# machine-drafted, needs native review
# DJI radiometric JPEGs (drone thermal cameras): the photo and metadata open; the
# temperatures would need DJI's SDK.
decoders-dji-thermal-why = DJI 的 SDK 许可只允许随附该 SDK 的程序使用它，不允许使用你自行安装的副本。照片及其元数据仍可打开。
decoders-occt-why = STEP 和 IGES 实体需要 OpenCASCADE，而此版本的 { -app-name } 无法使用它。
# Rows with one button each, under a decoder.
decoders-get-row = 开发商的下载页面
decoders-get = 获取
decoders-locate-row = 安装在别处？
decoders-locate = 定位…
decoders-forget-row = 这是你手动选择的。
decoders-forget = 清除…
# Asked before Forget… does anything.
decoders-forget-ask = 清除你手动选择的 { $decoder }？
decoders-forget-detail = { -app-name } 会重新自行查找它。磁盘上的内容不会被删除。
decoders-forget-ok = 清除
# The title of the Locate… file dialog.
decoders-locate-title = 定位 { $decoder }
# Toasts.
decoders-locate-wrong = 这不是 { $decoder }。
decoders-located = 正在使用你选择的 { $decoder }。
decoders-forgotten = { -app-name } 重新自行查找 { $decoder }。
decoders-still-missing = 仍未找到 { $decoder }。
decoders-now-found = 已找到 { $decoder }。
decoders-looked = 已重新查找解码器。

## The card shown in place of a file that needs one. $file is the file's name.

decoders-card-title = { $format } 需要 { $vendor } 的免费解码器
decoders-card-what = { $file } 需用 { $decoder } 打开。它是免费的。
decoders-card-how = 安装后回到这里：{ -app-name } 会自行找到它。
decoders-card-get = 获取
decoders-card-installed = 我已安装
decoders-card-locate = 定位…

## The gallery: the badge on such a file's thumbnail, and a line of its tooltip.

gallery-badge-needs-decoder = 需要解码器
gallery-needs-decoder = 需用 { $vendor } 的免费 { $decoder } 打开，但尚未安装。

## Why a file did not open, after "Can't open FILE:".

error-reason-needs-decoder = { $format } 文件需要 { $vendor } 的免费 { $decoder }，但尚未安装。
