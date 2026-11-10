# machine-drafted, needs native review
# Specular — 开始屏幕（Specular 不带文件打开时以及关闭最后一张图像时显示）、“试试这些”卡片和只显示一次的提示。简体中文。

## 开始屏幕

welcome-title = { -app-name }
welcome-tagline = 快速、忠实的查看器，适用于照片以及技术、科学和医学图像。
welcome-open = 打开文件…
welcome-open-about = 图像、文档或数据文件。
welcome-browse = 浏览文件夹…
welcome-browse-about = 选择一个文件夹，以画廊方式查看。
welcome-pictures = 你的图片
welcome-pictures-about = 图库中的所有照片，按日期和地点排列。
welcome-pictures-setup-title = 设置图库…
welcome-pictures-setup = 从你的“{ $folder }”文件夹开始。
welcome-pictures-ask = 将你的 { $folder } 文件夹添加到图库吗？
welcome-pictures-add = 添加文件夹
welcome-drop = 你也可以将文件或文件夹拖放到此窗口的任意位置。
welcome-recent = 最近的文件夹
welcome-recent-name = { $name }，位于 { $parent }
welcome-tips = 试试这些
welcome-tips-hide = 隐藏这些提示
welcome-tips-hidden = 提示已隐藏。命令面板中的“显示欢迎页”可将其恢复。

## 试试这些：每张卡片一个名称和一行说明。名称旁的按键是用户自己的绑定。

welcome-tip-gallery = 画廊
welcome-tip-gallery-about = 一眼查看整个文件夹。
welcome-tip-compare = 比较
welcome-tip-compare-about = 在画廊中选择两到四张图像，并排查看。
welcome-tip-cull = 筛选模式
welcome-tip-cull-about = 每个操作一个键：挑选、排除并评分一组照片。
welcome-tip-slideshow = 幻灯片放映
welcome-tip-slideshow-about = 全屏逐张显示一个文件夹。
welcome-tip-palette = 命令面板
welcome-tip-palette-about = 按名称查找任意命令或设置。
welcome-tip-studies = 检查
welcome-tip-studies-about = DICOM 光盘或文件夹中的患者、检查和序列。
welcome-tip-dicom-servers = DICOMweb 服务器
welcome-tip-dicom-servers-about = 浏览并打开 PACS 中的检查。
welcome-tip-dicom-tags = DICOM 标签
welcome-tip-dicom-tags-about = 图像的所有标签，可搜索。
welcome-tip-window-level = 窗宽/窗位
welcome-tip-window-level-about = 拖动以调整 DICOM 图像的对比度。
welcome-tip-omero-servers = OMERO 服务器
welcome-tip-omero-servers-about = 浏览 OMERO 和 IDR 上的显微镜项目。
welcome-tip-stretch = 拉伸
welcome-tip-stretch-about = 显现天文和科学数据中的微弱细节。
welcome-tip-drawing-layers = 图纸图层
welcome-tip-drawing-layers-about = 显示和隐藏技术图纸的图层。
welcome-tip-needs-image = 请先打开一张图像，然后按 { $key }。
welcome-tip-needs-gallery = 在画廊中选择两到四张图像，然后按 { $key }。
welcome-tip-key = 快捷键：{ $key }

## 一次性提示：每条只显示一次，不打扰，每次启动最多一条。设置 › 通用 › 显示提示 可全部关闭。

hint-gallery = 按 { $key } 以画廊方式查看此文件夹。
hint-gallery-first = 选择两到四张图像，按 { $compare } 进行比较，或按 { $cull } 筛选此文件夹。
hint-raw = 这是一个 RAW 照片文件夹：按 { $key } 进行筛选（挑选、排除和评分）。
hint-pages =
    { $count ->
       *[other] 此文件有 { $count } 页。{ $next } 和 { $prev } 用于翻页。
    }
hint-turn-off = 关闭提示
hint-turned-off = 提示已关闭。可在 设置 › 通用 › 显示提示 中重新打开。

## 命令面板

palette-show-welcome = 显示欢迎页
palette-show-welcome-help = 开始屏幕，包含打开、浏览、你的图片和可以试试的功能。
palette-close = 关闭
palette-close-help = 关闭图片或文件夹并返回开始屏幕；在开始屏幕上则退出。

## 首次运行（在使用场景问题下方）

profile-on-start = { -app-name } 不带文件打开时：
profile-skip = 跳过
