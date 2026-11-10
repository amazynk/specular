# Specular — 客户画廊：静态 HTML 照片文件夹，以及读回客户的选片。

# machine-drafted, needs native review

palette-client-gallery = 客户画廊…
palette-client-gallery-help = 将照片写成供客户浏览的网页文件夹，客户可以把选片发回。
palette-import-client-picks = 导入客户选片…
palette-import-client-picks-help = 标记客户在画廊中选中的照片。

webgallery-heading = 客户画廊
webgallery-subtitle = { $count ->
   *[other] { $count } 张照片
}
webgallery-title-label = 标题
webgallery-title-default = 样片
webgallery-watermark-label = 水印文字（留空：无）
webgallery-source = 照片
webgallery-source-current = 这张照片
webgallery-source-selection = 所选内容
webgallery-source-album = 相册“{ $name }”
webgallery-source-search = 搜索结果
webgallery-source-folder = 此文件夹
webgallery-source-picks = 选用
webgallery-source-value = { $what }（{ $count }）
webgallery-theme = 主题
webgallery-theme-light = 浅色
webgallery-theme-dark = 深色
webgallery-theme-minimal = 简约
webgallery-size = 长边
webgallery-size-value = { $px } 像素
webgallery-format = 格式
webgallery-captions = 每张照片下方
webgallery-captions-off = 无
webgallery-captions-name = 文件名
webgallery-captions-title = 标题
webgallery-captions-all = 文件名、标题和说明
webgallery-exif = 显示相机和曝光
webgallery-location = 在照片中保留位置（GPS）
webgallery-note-private = 静态画廊无法用密码保护。
webgallery-note-share = 拥有该文件夹或其地址的任何人都能看到所有照片。只与客户分享链接，或使用网站托管服务的访问控制。
webgallery-note-picks = 客户用爱心选片。选片会保留在客户的浏览器中，直到他们把列表发给你。
webgallery-write = 写出画廊…
webgallery-busy = 正在写入第 { $done } 张，共 { $total } 张…
webgallery-stop = 停止
webgallery-done = { $count ->
   *[other] 画廊已写出：{ $folder } 中有 { $count } 张照片
}
webgallery-done-failed = { $failed ->
   *[other] 有 { $failed } 张照片无法写入。
}
webgallery-stopped = 画廊已停止，未保留任何内容。
webgallery-failed = 画廊未写出：{ $reason }
webgallery-nothing = 没有任何照片能够写入。

webgallery-page-count = { $count ->
   *[other] { $count } 张照片
}
webgallery-page-hint = 点按你想要的照片上的爱心。在你发送之前，选片会保留在此浏览器中。
webgallery-page-picked = 已选：{"{"}n{"}"}
webgallery-page-pick = 选择
webgallery-page-copy = 复制我的选片
webgallery-page-copied = 已复制。请把列表粘贴到消息中。
webgallery-page-copy-by-hand = 复制选中的列表并粘贴到消息中。
webgallery-page-download-txt = 下载 picks.txt
webgallery-page-download-json = 下载 picks.json
webgallery-page-clear = 清除选片
webgallery-page-clear-ask = 清除此浏览器中的全部选片？
webgallery-page-no-storage = 此浏览器不会保存你的选片。请在关闭页面前发送。
webgallery-page-noscript = 选片需要 JavaScript。照片仍可打开。
webgallery-page-previous = 上一张
webgallery-page-next = 下一张
webgallery-page-close = 关闭
webgallery-page-position = 第 {"{"}i{"}"} 张，共 {"{"}n{"}"} 张
webgallery-page-viewer = 照片查看器
webgallery-page-picks = 你的选片

clientpicks-heading = 导入客户选片
clientpicks-subtitle = { $count ->
   *[other] 与显示的 { $count } 张照片比对
}
clientpicks-paste-label = 名称（粘贴客户的列表）
clientpicks-choose = 选择选片文件…
clientpicks-from-file = 来自 { $name }
clientpicks-apply-as = 标记为
clientpicks-as-pick = 选用
clientpicks-as-stars = { $stars ->
   *[other] { $stars } 星
}
clientpicks-as-label = 标签 { $label }
clientpicks-empty = 粘贴客户发来的列表，或选择其 picks.txt 或 picks.json。
clientpicks-matched = { $total } 个名称中有 { $matched } 个与此处的照片匹配。
clientpicks-missing = 未找到：{ $names }
clientpicks-ambiguous = 位于多个文件夹，未作更改：{ $names }
clientpicks-more = { $names } 等另外 { $more } 个
clientpicks-apply = 应用
clientpicks-applied = { $count ->
   *[other] 已标记 { $count } 张照片。
}
clientpicks-unchanged = 这些照片已经标记过了。
clientpicks-read-failed = 无法读取 { $name }：{ $reason }
