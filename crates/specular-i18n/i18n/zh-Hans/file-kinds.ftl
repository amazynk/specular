# Specular — File kinds: which format a shared extension is, Open as…, remembered choices. 简体中文.

## Which kind of file? Files whose extension several formats share.

# machine-drafted, needs native review
file-kind-title = { $file } 是哪种文件？
# machine-drafted, needs native review
file-kind-why-content = 其内容符合不止一种格式。
# machine-drafted, needs native review
file-kind-why-name = 其内容无法说明。这些格式都使用 .{ $ext } 文件。
# machine-drafted, needs native review
file-kind-why-none = 没有格式能识别其内容。
# machine-drafted, needs native review
file-kind-certain = 确定
# machine-drafted, needs native review
file-kind-likely = 很可能
# machine-drafted, needs native review
file-kind-possible = 可能
# machine-drafted, needs native review
file-kind-guess = 仅凭名称
# machine-drafted, needs native review
file-kind-hint-same = 与此处其他 .{ $ext } 文件相同
# machine-drafted, needs native review
file-kind-hint-companions = 其常用的配套文件就在旁边
# machine-drafted, needs native review
file-kind-scope = 应用于
# machine-drafted, needs native review
file-kind-scope-file = 此文件
# machine-drafted, needs native review
file-kind-scope-folder = 此文件夹中的 .{ $ext } 文件
# machine-drafted, needs native review
file-kind-scope-everywhere = 所有 .{ $ext } 文件
# machine-drafted, needs native review
file-kind-later = 以后可在文件菜单的“打开为…”中更改。
# machine-drafted, needs native review
file-kind-open = 打开
# machine-drafted, needs native review
file-kind-raw = 原始样本
# machine-drafted, needs native review
file-kind-open-as-none = 没有其他方式能读取 { $file }。
# machine-drafted, needs native review
error-reason-ambiguous = 它可能是几种格式之一。请选择一种来打开。

## Open as… (the palette and the gallery's file menu)

# machine-drafted, needs native review
palette-open-as = 打开为…
# machine-drafted, needs native review
palette-open-as-help = 选择以哪种格式读取该文件。

## Settings › Optional decoders › File kinds

# machine-drafted, needs native review
settings-decoders-file-kinds = 清除所有文件类型…
# machine-drafted, needs native review
settings-decoders-file-kinds-help = 你为多种格式共用的扩展名所选的格式。下次会再次询问。
# machine-drafted, needs native review
file-kind-rules-intro = 每条规则把某个扩展名的文件按一种格式打开。
# machine-drafted, needs native review
file-kind-rules-none = 还没有规则。询问时请选择“此文件夹中的 .ext 文件”或“所有 .ext 文件”。
# machine-drafted, needs native review
file-kind-rule = .{ $ext } 文件：{ $format }
# machine-drafted, needs native review
file-kind-rule-folder = 在 { $folder } 中
# machine-drafted, needs native review
file-kind-rule-everywhere = 所有位置
# machine-drafted, needs native review
file-kind-forget = 清除…
# machine-drafted, needs native review
file-kind-forget-ask = 要清除“.{ $ext } 文件：{ $format }”吗？
# machine-drafted, needs native review
file-kind-forget-detail = 下次 { -app-name } 会再次询问。不会更改或删除任何文件。
# machine-drafted, needs native review
file-kind-forget-ok = 清除
# machine-drafted, needs native review
file-kind-forgotten = 已清除 .{ $ext } 文件：{ $format }。
# machine-drafted, needs native review
file-kind-forget-all-ask = { $count ->
   *[other] 要清除 { $count } 条文件类型规则吗？
}
# machine-drafted, needs native review
file-kind-forgotten-all = { $count ->
   *[other] 已清除 { $count } 条文件类型规则。
}
# machine-drafted, needs native review
file-kind-keywords = 扩展名, 格式, 打开为, 哪种, 文件类型

## One line on what each format is, under its name in the prompt.

# machine-drafted, needs native review
file-kind-about-other = 另一种使用 .{ $ext } 文件的格式。
# machine-drafted, needs native review
file-kind-about-raw-volume = 无文件头的样本。需要你提供宽度、高度、层面数和样本类型。
# machine-drafted, needs native review
file-kind-about-raw = 相机原始照片。
# machine-drafted, needs native review
file-kind-about-flir-fff = 带温度数据的 FLIR 热成像图像。
# machine-drafted, needs native review
file-kind-about-fits = 天文图像（FITS）。
# machine-drafted, needs native review
file-kind-about-fit = Garmin 运动轨迹：GPS 点和传感器数据。
# machine-drafted, needs native review
file-kind-about-mif = MRtrix 医学体数据（MRI）。
# machine-drafted, needs native review
file-kind-about-map-info-mif = MapInfo 矢量地图，带其 .mid 数据。
# machine-drafted, needs native review
file-kind-about-net-cdf = 科学网格数据（气候、海洋）。
# machine-drafted, needs native review
file-kind-about-hdf5 = HDF5 科学数据。
# machine-drafted, needs native review
file-kind-about-playlist = 音频播放列表（PLS）。
# machine-drafted, needs native review
file-kind-about-gerber = 电路板层（Gerber），例如锡膏层。
# machine-drafted, needs native review
file-kind-about-ser = 行星相机拍摄的天文视频（SER）。
# machine-drafted, needs native review
file-kind-about-tia = 电子显微镜序列（FEI TIA）。
file-kind-about-pcd = 3D 点云（PCD）。
file-kind-about-photo-cd = Kodak Photo CD 扫描。
file-kind-about-zx-screen = ZX Spectrum 屏幕转储。
file-kind-about-cpc-screen = Amstrad CPC 屏幕转储。
file-kind-about-msx-screen = MSX 屏幕转储。
file-kind-about-scc = Scenarist 隐藏式字幕。
# machine-drafted, needs native review
file-kind-about-pict = 经典 Mac 图片（PICT）。
# machine-drafted, needs native review
file-kind-about-hdr = Radiance HDR 图像。
# machine-drafted, needs native review
file-kind-about-analyze = Analyze 7.5 医学体数据，带其 .hdr。
# machine-drafted, needs native review
file-kind-about-envi = 遥感栅格（ENVI），带其 .hdr。
# machine-drafted, needs native review
file-kind-about-pds3 = 行星任务图像（PDS3），带其标签。
# machine-drafted, needs native review
file-kind-about-mpeg-ps = 摄像机视频（MPEG 节目流，MOD/TOD）。
# machine-drafted, needs native review
file-kind-about-tracker = Tracker 音乐模块（MOD、S3M、XM、IT、669、MTM、STM、OKT、MED、ULT、FAR）。
# machine-drafted, needs native review
file-kind-about-softimage-pic = Softimage 3D 渲染图（PIC）。
# machine-drafted, needs native review
file-kind-about-dr-halo = Dr. Halo 图片，带其 .pal 调色板。
# machine-drafted, needs native review
file-kind-about-wal = Quake 2 纹理，按游戏调色板着色。
# machine-drafted, needs native review
file-kind-about-quick-time = QuickTime 影片。
file-kind-about-pro-res-raw = Apple ProRes RAW 视频：相机传感器帧。
file-kind-about-cinema-raw-light = Canon Cinema RAW Light 视频：相机传感器帧。
# machine-drafted, needs native review
file-kind-about-gimp-brush = GIMP 画笔。
# machine-drafted, needs native review
file-kind-about-swatch-exchange = Adobe Swatch Exchange 调色板。
# machine-drafted, needs native review
file-kind-about-aseprite = Aseprite 像素画精灵。
