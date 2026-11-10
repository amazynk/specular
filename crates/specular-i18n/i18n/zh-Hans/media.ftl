# 动画播放与阅读模式（压缩包、漫画）。

disabled-not-animated = 此图像不是动画
disabled-in-archive = 压缩包内的页面无法移到废纸篓

## 命令面板与快捷键一览

palette-play-pause = 播放或暂停动画
palette-next-frame = 下一帧
palette-prev-frame = 上一帧
palette-faster = 加快播放
palette-slower = 减慢播放
palette-normal-speed = 正常播放速度
palette-loop-forever = 无限循环动画
palette-right-to-left = 从右到左的页序
# machine-drafted, needs native review
palette-contents = 目录
palette-spreads = 双页显示

## 动画控制

transport-region = 动画控制
transport-play = 播放
transport-pause = 暂停
transport-prev-frame = 上一帧
transport-next-frame = 下一帧
transport-slower = 减慢
transport-faster = 加快
transport-frame = 第 { $index } / { $count } 帧
transport-frame-unknown = 第 { $index } 帧
transport-speed = { $speed }×
transport-speed-name = 播放速度 { $speed }×
# machine-drafted, needs native review
motion-badge = 动态
motion-badge-live = 实况
motion-play = 播放动态视频
motion-play-live = 播放实况照片
motion-stop = 返回静态照片
toast-motion-failed = 无法播放动态视频：{ $reason }

## 通知

toast-speed = 播放速度 { $speed }×
toast-loop-forever-on = 无限循环
toast-loop-forever-off = 按文件设定循环
toast-right-to-left-on = 页面从右到左阅读
toast-right-to-left-off = 页面从左到右阅读
toast-spreads-on = 双页显示
toast-spreads-off = 单页显示

## 压缩包

title-archive-page = { $page } — { $archive }
readout-position-spread = { $first }–{ $last } / { $count }
archive-needs-sandbox = { $format } 压缩包只能由沙盒解码进程打开，此版本尚未包含该进程。
archive-empty = 此压缩包中没有图像。
archive-failed = 无法打开压缩包：{ $error }
archive-encrypted = 此压缩包已加密。请再次打开它以输入密码。
archive-wrong-password = 此压缩包的密码错误。
password-title = 需要密码
password-archive = “{ $archive }”已加密。
password-label = 密码
password-show = 显示密码
password-wrong = 密码错误，请重试。
password-open = 打开
password-cancel = 取消

## 设置

settings-viewing-animation-autoplay = 自动播放动画
settings-viewing-animation-autoplay-help = 打开动画图像（GIF、APNG、WebP 等）后立即播放。
settings-viewing-animation-loop = 动画循环
settings-viewing-animation-loop-help = 动画重复的次数。
settings-viewing-animation-loop-file = 按文件设定
settings-viewing-animation-loop-forever = 无限
settings-viewing-animation-loop-once = 只播放一次
settings-viewing-animation-short-delays = 极短的帧间隔
settings-viewing-animation-short-delays-help = 设定为 10 毫秒或更短的帧：像网页浏览器一样放慢到 100 毫秒，或按保存的间隔播放。
settings-viewing-animation-short-delays-browser = 与网页浏览器相同（100 毫秒）
settings-viewing-animation-short-delays-as-stored = 按保存的间隔
settings-browsing-reading-order = 压缩包中的页序
settings-browsing-reading-order-help = 当 ComicInfo.xml 注明时（日漫），漫画压缩包从右到左阅读，除非在此另行设置。
settings-browsing-reading-order-auto = 按压缩包
settings-browsing-reading-order-left-to-right = 从左到右
settings-browsing-reading-order-right-to-left = 从右到左
settings-browsing-page-layout = 压缩包中的页面
settings-browsing-page-layout-help = 显示单页或并排显示两页；封面始终单独显示。
settings-browsing-page-layout-auto = 按压缩包
settings-browsing-page-layout-single = 单页
settings-browsing-page-layout-spreads = 双页

## Pages of multi-image files (ICNS sizes, texture layers, faces and slices, TIFF pages) (machine-drafted, needs native review)
palette-next-page = 下一页
palette-prev-page = 上一页
palette-first-page = 第一页
palette-last-page = 最后一页
disabled-one-page = 此图像只有一页
page-indicator = 第 { $index } 页，共 { $count } 页
page-indicator-labelled = { $page }，{ $label }
page-label-icon = { $width }×{ $height }
page-label-icon-density = { $width }×{ $height } @{ $density }x
page-label-layer = 层 { $index }
page-label-face = 面 { $face }
page-label-sphere = 球面贴图
page-label-slice = 层面 { $index }
page-label-join = { $a } · { $b }
page-label-slide = 玻片
page-label-slide-label = 标签
page-label-slide-macro = 概览
page-label-slide-thumbnail = 缩略图
# machine-drafted, needs native review
page-label-thermal = 热图像
page-label-visible = 可见光照片
# machine-drafted, needs native review
page-label-left-view = 左视图
# machine-drafted, needs native review
page-label-right-view = 右视图
# machine-drafted, needs native review
# An object of a CERN ROOT file in the page picker: its path in the file and its class, e.g.
# "run1/hits (TH2D)".
page-label-object = { $name }（{ $class }）
# machine-drafted, needs native review
# A view of one step of a circuit board job ("panel · Top").
page-label-board-step = { $step } · { $view }
# machine-drafted, needs native review
# A focal plane and a time point of a remote microscopy image (OMERO), counted from 1.
page-label-z = Z { $index } / { $count }
page-label-t = T { $index } / { $count }
# machine-drafted, needs native review
# Views of a remote-sensing product (an ENVI cube, a Sentinel-2 product) in the page picker.
page-label-product-composite = 默认波段
page-label-product-bands = 全部波段
page-label-product-true-colour = 真彩色
page-label-product-false-colour = 假彩色（近红外）
page-label-product-swir = 短波红外
page-label-product-agriculture = 农业
page-label-product-scene-class = 场景分类
page-label-product-cloud = 云概率
# A product view at a pixel size, e.g. "True colour, 10 m".
page-label-product-at = { $view }，{ $metres } m
title-file-page = { $file } — { $page }
info-page-group = 页面
info-page-position = 位置
info-page-contents = 内容

# machine-drafted, needs native review
# OpenColorIO (Settings › Colour): why scene-linear images are shown without it.
ocio-failed = 未使用 OCIO：{ $reason }
ocio-builtin-config = $OCIO 指向内置配置（ocio://）；请将其另存为文件，然后选择该文件。

## Page labels for icons and collections (machine-drafted, needs native review)
page-label-size = { $width }×{ $height }
page-label-size-bits = { $width }×{ $height } · { $bits } 位

## Page labels for Photoshop layers (machine-drafted, needs native review)
page-label-merged = 合并图像
page-label-from-layers = 由图层合成
page-label-layer-hidden = { $name }（隐藏）
page-label-layer-unnamed = 未命名图层 { $index }
