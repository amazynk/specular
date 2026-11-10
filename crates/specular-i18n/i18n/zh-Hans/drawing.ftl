# Specular — Drawings (DXF): layers, paper, lineweights, model space and layouts. 简体中文.

keys-mode-drawing = 图纸
cheatsheet-drawing = 图纸
palette-drawing-layers = 显示或隐藏图层面板
palette-drawing-all-layers = 显示所有图层，或按保存时的图层
palette-drawing-paper = 切换深色和浅色纸张
palette-drawing-lineweights = 显示或隐藏线宽
disabled-not-drawing = 仅适用于图纸 (DXF)
# A drawing's page: model space (layouts are shown by their own name).
page-label-model = 模型
# machine-drafted, needs native review
cad-pen-name = 笔 { $n }
cad-page-name = 页 { $n }
cad-layer-name = 图层 { $n }
cad-map-name = 地图
# machine-drafted, needs native review
# A board's views (its Gerber files together): every layer stacked, and its top and bottom
# in board colours.
cad-board-layers-name = 层
cad-board-top-name = 顶面
cad-board-bottom-name = 底面
# machine-drafted, needs native review
# A plot's layers: its frame, a trace without a name (numbered from 1), and the contour
# lines of a 2-D spectrum above and below zero.
cad-chart-frame-name = 边框
cad-trace-name = 曲线 { $n }
cad-contours-positive-name = 正等高线
cad-contours-negative-name = 负等高线
# machine-drafted, needs native review
# The page of a survey file (LandXML) showing its surfaces and alignments in 3D.
cad-model-3d-name = 三维模型
# The layers panel's title (and its name for screen readers).
drawing-layers-title = 图层 · 显示 { $shown }/{ $total }
# Toasts.
drawing-layers-all-toast = 显示所有图层
drawing-layers-saved-toast = 按图纸中保存的图层显示
drawing-paper-dark-toast = 深色纸张
drawing-paper-light-toast = 浅色纸张
drawing-lineweights-on = 已显示线宽
drawing-lineweights-off = 线宽已关闭：细线

# machine-drafted, needs native review
# Markup on drawings and images (Alt+M): kept in a file next to the drawing, never in it.
palette-markup = 添加标记
palette-markup-help = 云线、文字、箭头、图章、测量和照片图钉，保存在文件旁边。
palette-compare-sheets = 比较图纸
palette-compare-sheets-help = 叠加此图纸的另一版本，自动对齐，并圈出变更。
markup-title = 标记
markup-list = 本页标记
markup-count = { $count ->
    [0] 本页没有标记
   *[other] 本页有 { $count } 项标记
}
markup-empty = 选择工具后在图纸上单击。
markup-item = { $n }. { $kind }
markup-tool-select = 选择
markup-tool-cloud = 云线
markup-tool-text = 文字
markup-tool-arrow = 箭头
markup-tool-stamp = 图章
markup-tool-length = 长度
markup-tool-area = 面积
markup-tool-pin = 照片图钉
markup-hint-select = 单击项目以选中，拖动以移动。按 Enter 编辑文字。
markup-hint-cloud = 拖出一个矩形，或单击各角后按 Enter。
markup-hint-text = 在放置文字的位置单击。
markup-hint-arrow = 从标签拖到指向的点，或分别单击两处。
markup-hint-stamp = 在放置图章的位置单击。
markup-hint-length = 拖动，或单击两端。
markup-hint-area = 单击各角，然后按 Enter。
markup-hint-pin = 单击一个点，然后选择照片。
markup-kind-cloud = 云线
markup-kind-text = 文字
markup-kind-arrow = 箭头
markup-kind-stamp = 图章
markup-kind-length = 长度
markup-kind-area = 面积
markup-kind-pin = 照片
markup-colour = 颜色：{ $colour }
markup-colour-red = 红色
markup-colour-blue = 蓝色
markup-colour-green = 绿色
markup-colour-orange = 橙色
markup-colour-magenta = 品红
markup-colour-black = 黑色
markup-width = 线宽：{ $width }
markup-width-thin = 细
markup-width-medium = 中
markup-width-thick = 粗
markup-stamp = 图章：{ $words }
markup-stamp-approved = 已批准
markup-stamp-rejected = 已驳回
markup-stamp-reviewed = 已审核
markup-stamp-custom = 自定义文字
markup-undo = 撤销
markup-redo = 重做
markup-delete = 删除
markup-open-photo = 打开照片
markup-from-measurements = 添加测量
markup-from-measurements-done = { $count ->
   *[other] 已将 { $count } 项测量添加为标记
}
markup-export-pdf = 导出 PDF…
markup-export-png = 导出 PNG…
markup-done = 完成
markup-ask-text = 文字（Enter 放置，Esc 取消）
markup-ask-label = 标签（可选；Enter 放置）
markup-ask-stamp = 图章文字（Enter 放置）
markup-ask-edit = 新文字（Enter 保存）
markup-added = 已添加{ $kind }
markup-moved = 已移动
markup-pin-hint = 再次单击图钉或按 Enter 打开其照片。
markup-deleted = 已删除
markup-undone = 已撤销
markup-redone = 已重做
markup-delete-ask = 删除{ $name }？
markup-delete-detail = 将从标记文件中移除。图纸本身不会更改。
markup-delete-ok = 删除
markup-photo-missing = { $name } 已不存在。
markup-load-failed = 未能读取标记：{ $reason }
markup-save-failed = 未能保存标记：{ $reason }
markup-sidecar-too-large = 标记文件过大
markup-pdf-unreadable = 无法读取该 PDF
markup-exported = 已保存 { $name }
markup-export-failed = 未导出：{ $reason }
markup-export-same-file = 请换一个名称；原文件永远不会被覆盖
markup-export-encrypted = 该 PDF 已加密。请改为导出 PNG。
markup-export-damaged = 该 PDF 的结构已损坏。请改为导出 PNG。
sheets-title = 比较图纸
sheets-other = 另一张图纸…
sheets-other-named = 另一张：{ $name }
sheets-page = 第 { $page } 页，共 { $count } 页
sheets-auto = 对齐
sheets-pick = 选取 2 个点
sheets-overlay = 叠加
sheets-difference = 差异
sheets-blend = 渐变
sheets-blink = 闪烁
sheets-slider = 另一张图纸 { PERCENT($percent) }
sheets-clouds = { $count ->
    [0] 无变更
   *[other] 用云线圈出 { $count } 处变更
}
sheets-done = 完成
sheets-choose = 选择另一张图纸。
sheets-reading = 正在读取图纸…
sheets-aligning = 正在对齐…
sheets-aligned = 已对齐：比例 { $scale }，旋转 { $angle }°，误差在 { $error } px 以内。{ $changes ->
    [0] 未发现变更。
   *[other] { $changes } 处变更。
}
sheets-align-failed = 无法自动对齐。请选取 2 个点。
sheets-pick-this = 点 { $n }：在此图纸上单击该点。
sheets-pick-other = 点 { $n }：在另一张图纸上单击同一点。
sheets-pick-same = 两个点相同。请重新选取 2 个点。
sheets-read-failed = 未能读取图纸：{ $reason }
sheets-failed = 该步骤失败。请重试或选择另一张图纸。
sheets-legend-this = 仅此图纸
sheets-legend-other = 仅另一张图纸
sheets-legend-both = 两者
sheets-cloud-text = 已更改
sheets-clouds-added = { $count ->
    [0] 未添加云线
   *[other] 已向标记添加 { $count } 个云线
}
sheets-opened = 比较图纸
sheets-closed = 已关闭比较
markup-ask-new-line = Shift+Enter：换行

## Settings › Drawings

# machine-drafted, needs native review
settings-section-drawings = 图纸
settings-drawings-shx-folders = SHX 字体文件夹
settings-drawings-shx-folders-help = 存放 DXF 和 DWG 文字所用 SHX 字体的文件夹，以 ; 分隔。首先搜索图纸本身所在的文件夹。
# A folder in the SHX font folders setting that does not exist or is not a folder.
settings-error-shx-folder = 不是文件夹：{ $folder }
