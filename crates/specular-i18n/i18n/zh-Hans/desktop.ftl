# 桌面深度集成：设置 → 集成，设为壁纸。

settings-section-integration = 集成
settings-integration-default-viewer = 默认图像查看器
settings-integration-default-viewer-help = 双击图像时用 { -app-name } 打开。移除后会恢复之前的查看器。
# machine-drafted, needs native review
settings-integration-extra-default-types = 更多文件类型
settings-integration-extra-default-types-help = 双击时用 { -app-name } 打开的其他文件类型，在“默认图像查看器”下选择。
settings-integration-thumbnailer = 文件管理器缩略图
settings-integration-thumbnailer-help = 让文件管理器为它们自己无法读取的格式显示预览，例如 JPEG XL、HEIC 和相机 RAW 文件。
settings-integration-single-instance = 通过 D-Bus 单实例运行
settings-integration-single-instance-help = 从文件管理器、启动器或终端打开的文件会发送到正在运行的 { -app-name } 窗口。对之后打开的窗口生效。
settings-integration-file-manager-actions = 文件管理器操作
settings-integration-file-manager-actions-help = 在 Nautilus、Dolphin、Thunar 和 Nemo 的右键菜单中添加“打开”“比较”和“设为壁纸”。
settings-integration-search-provider = 桌面搜索
# machine-drafted, needs native review
settings-integration-search-provider-help = 在 GNOME 和 KDE 的搜索中查找资料库里的图片。搜索词的用法与资料库搜索栏相同。
# machine-drafted, needs native review
settings-integration-url-scheme = 打开 specular:// 链接
settings-integration-url-scheme-help = 浏览器或其他程序中的 specular:///home/me/photo.jpg 之类的链接会在 { -app-name } 中打开。只打开这台电脑上的文件，异常文件需您确认后才打开。

integration-state-on = 已开启
integration-state-off = 已关闭
integration-state-partial = 仅部分设置。再次应用即可修复。
integration-state-unavailable = 在此桌面上不可用
integration-state-sandboxed = 在沙盒 (Flatpak) 中不可用
integration-state-no-bus = 不可用：没有 D-Bus 会话
integration-state-no-file-manager = 不可用：未找到受支持的文件管理器
integration-state-no-search = 不可用：未找到 GNOME Shell 或 KRunner
integration-state-working = 正在处理…
integration-state-details = { $state } · { $details }
integration-detail-types = 图像类型：{ $count }/{ $total }
integration-detail-thumbnail-types = 图像类型：{ $count }
integration-apply = 应用
integration-remove = 移除
integration-done-on = “{ $option }”已开启
integration-done-off = “{ $option }”已关闭
integration-failed = 无法更改“{ $option }”：{ $reason }
# machine-drafted, needs native review
integration-search-remove-ask = 关闭桌面搜索？
integration-search-remove-detail = { -app-name } 将从 { $desktops } 的搜索中移除。你的资料库和图片会保留。
search-provider-open-folder = 打开文件夹

integration-label-open = 用 { -app-name } 打开
integration-label-compare = 用 { -app-name } 比较
integration-label-wallpaper = 设为壁纸
integration-label-generic-name = 图像查看器
integration-label-comment = 即刻打开并浏览大尺寸照片

palette-set-wallpaper = 设为壁纸
wallpaper-set = 已设置壁纸（{ $method }）
wallpaper-failed = 无法设置壁纸：{ $reason }

# machine-drafted, needs native review
url-invalid = 无法打开此 { -app-name } 链接：{ $reason }
url-not-found = 此链接指向的文件不存在
url-remote-refused = { -app-name } 只打开这台电脑上的文件，不打开 { $source }
url-confirm-files = { $more ->
    [0] “{ $file }”
   *[other] “{ $file }”等 { $more } 个其他文件
}
url-confirm-title = { $reason ->
    [dots] 打开 { url-confirm-files }？链接通过“..”文件夹到达它。
    [network] 打开 { url-confirm-files }？它位于网络共享或设备上。
   *[hidden] 打开 { url-confirm-files }？其名称包含不可见字符或文本方向字符。
}
url-confirm-open = 打开

# Launcher actions and recent files: machine-drafted, needs native review
integration-label-new-window = 新建窗口
integration-label-library = 图库
integration-label-slideshow = 图片幻灯片
settings-integration-recent-files = 桌面的最近文件
settings-integration-recent-files-help = 打开的图像会显示在文件管理器的“最近”中以及桌面的最近文件列表中。移除后，{ -app-name } 的条目会从这些列表中删除。
integration-detail-recent = 文件：{ $count }
# machine-drafted, needs native review
integration-extra-title = 更多文件类型
# machine-drafted, needs native review
integration-extra-intro = 除图片外，{ -app-name } 还可以在双击时打开这些文件。除非你勾选，否则不会勾选任何项。编辑这些文件的程序仍保留在“打开方式”中，取消勾选会把该类型交还给原来的程序。Blackmagic RAW、DWG、STEP、IGES 和 Zeiss CZI 还需要其开发商的免费软件：请参阅“设置 › 可选解码器”。
integration-extra-chosen = { $count ->
    [0] 未选择
   *[other] 已选择 { $count } 种类型
}
integration-extra-count = { $on }/{ $total }
integration-extra-none-yet = 暂无
integration-extra-keywords = 文件类型 格式 文件关联 双击 打开方式 编辑器 科学 医学 CAD GIS 3D 三维 分子 视频 SVG PDF
integration-extra-group-editors = 编辑器文件
# machine-drafted, needs native review
integration-extra-group-editors-help = 绘画和绘图程序的文档：Krita、GIMP、Paint.NET、PaintShop Pro、Procreate、Affinity、OpenRaster、Illustrator、CorelDRAW、Clip Studio Paint、Sketch 和 Figma。双击显示图像；编辑仍在创建它的程序中进行。
integration-extra-group-science = 科学
integration-extra-group-science-help = 仪器和科研软件的文件：热像仪、显微镜、表面测量、HDF5、NetCDF、地震和气象数据。
integration-extra-group-medical = 医学
integration-extra-group-medical-help = 医学扫描和脑成像：DICOM、NIfTI、MINC、NRRD、纤维束成像和脑表面。
integration-extra-group-cad-gis = CAD 和 GIS
# machine-drafted, needs native review
integration-extra-group-cad-gis-help = 绘图仪图纸、技术插图、芯片版图和地图：HP-GL、CGM、GDSII、Shapefile、GeoPackage、KML、GPX、GeoJSON 和高程瓦片。DWG 用免费的 ODA File Converter 打开，STEP 和 IGES 用 OpenCASCADE 打开。
integration-extra-group-models = 3D 和分子
integration-extra-group-models-help = 3D 模型、建筑模型、点云和分子：STL、OBJ、glTF、PLY、3MF、X3D、IFC、LAS、E57、PDB、CIF 等。
integration-extra-group-videos = 视频
# machine-drafted, needs native review
integration-extra-group-videos-help = 视频：MP4、MOV、M4V、3GP、3G2、MKV、WebM、AVCHD（MTS、M2TS）、OGV、IVF 和 MXF。Blackmagic RAW 用免费的 Blackmagic RAW SDK 打开。
integration-extra-group-other-images = SVG 和其他图像
# machine-drafted, needs native review
integration-extra-group-other-images-help = 通常交给其他程序的图像和文档：SVG 图纸、PDF、XPS、DjVu、EPS、WMF、EMF、Visio、OpenDocument、WordPerfect Graphics、EPUB 图书和字体，以及 MNG、FLIC 和动画光标等动画。
integration-extra-group-text = 文本、表格和字幕
integration-extra-group-text-help = 纯文本和日志、NFO、Markdown、CSV 和 TSV 表格、RTF 以及字幕文件：SRT、WebVTT、ASS、SSA、MicroDVD、PGS、LRC、TTML 和 SCC。
