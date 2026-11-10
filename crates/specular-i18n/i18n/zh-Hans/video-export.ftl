# Specular — Export slideshow as video (DESIGN.md §6.5). Simplified Chinese. Machine-drafted, needs native review.

palette-export-slideshow = 将幻灯片放映导出为视频…

video-export-title = 将幻灯片放映导出为视频
video-export-resolution = 分辨率
# $width and $height in pixels.
video-export-resolution-window = 窗口（{ $width } × { $height }）
video-export-fps = 帧率
# $fps frames per second, a number.
video-export-fps-value = { $fps } fps
video-export-quality = 质量
video-export-quality-high = 高
video-export-quality-standard = 标准
video-export-quality-compact = 紧凑
video-export-images = { $count ->
   *[other] { $count } 张图像，采用幻灯片放映的顺序、过渡效果和说明文字
}
# $length a duration ("2:40"), $size a file size ("38 MB").
video-export-summary = 时长约 { $length }，大小约 { $size }
video-export-export = 导出…
video-export-replace = 替换
# $file a file name.
video-export-replace-question = { $file } 已存在。要替换吗？
# $percent a formatted percentage ("42%").
video-export-progress = 正在导出… { $percent }
video-export-cancelling = 正在取消…
# $file a file name, $size a file size ("38 MB").
video-export-saved = 已保存 { $file }（{ $size }）
video-export-skipped = { $count ->
   *[other] 有 { $count } 张图像无法读取，已略过。
}
video-export-failed = 无法导出视频：{ $reason }
video-export-cancelled = 已取消导出
video-export-no-images = 所有图像都无法读取。
video-export-show-in-folder = 在文件夹中显示

## Toolbar item and compatibility (machine-drafted, needs native review)
settings-viewing-toolbar-items-export-slideshow = 将幻灯片导出为视频
video-export-compatibility = 兼容性
video-export-compatibility-current = 当前播放器
video-export-compatibility-all = 所有播放器
