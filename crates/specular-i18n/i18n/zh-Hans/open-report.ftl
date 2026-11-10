# machine-drafted, needs native review
# Specular — 打开报告：图像为何花了这么久才打开（信息面板的“性能”部分；DESIGN.md §12.7）。

open-report-title = 性能

## Headlines
open-report-fast = 用时 { $ms } 毫秒打开
open-report-display-cache = 从显示缓存中显示
open-report-slow-read = 存储较慢：{ $percent } 的时间用于读取文件（{ $speed } MB/s）
# machine-drafted, needs native review
open-report-slow-read-no-speed = 存储较慢：{ $percent } 的时间用于读取文件（小文件：主要在等待存储响应）
open-report-archive-read = { $percent } 的时间用于解压归档
open-report-startup = { $percent } 的时间用于启动 { -app-name }
open-report-checked = 解码了两次：硬件解码器的首个结果已与 CPU 的结果比对（占 { $percent } 的时间）
open-report-hardware = 由硬件解码器解码，占 { $percent } 的时间
open-report-decode = 大图像：解码 { $mpx } 百万像素占 { $percent } 的时间
open-report-upload = { $percent } 的时间用于将图像发送到 GPU
open-report-waiting = { $percent } 的时间用于等待其他工作完成
open-report-stage = { $stage }占 { $percent } 的时间
open-report-memory-short = 内存不足，系统可能不得不使用交换空间

## Why a decode ran on the processor although a hardware decoder can do it
open-report-sw-not-probed = 软件解码：仍在检测硬件解码器
open-report-sw-no-device = 软件解码：此计算机上没有 { $api }
open-report-sw-no-codec = 软件解码：此 GPU 上的 { $api } 没有 { $codec } 解码器
open-report-sw-blocked-mismatch = 软件解码：硬件解码器的结果与 CPU 不一致，已被停用
open-report-sw-blocked-error = 软件解码：硬件解码器出错后已被停用
open-report-sw-blocked-lost = 软件解码：硬件解码器停止响应后已被停用
open-report-sw-failed = 软件解码：硬件解码器无法处理此图像
open-report-sw-busy = 软件解码：硬件解码器正忙
open-report-sw-unsupported = 软件解码：硬件解码器不支持此图像
open-report-sw-unavailable = 软件解码：在 { -app-name } 重新启动前，硬件解码已关闭
open-report-sw-cpu-only = 软件解码：处理位置设置为“仅 CPU”
open-report-sw-setting-off = 软件解码：硬件解码已在设置中关闭
open-report-sw-unchecked = 软件解码：硬件解码器尚未经过检查（会先在后台准备的图像上试用）
open-report-sw-lower-power = 软件解码：使用电池时，CPU 处理此任务更省电
open-report-sw-faster = 软件解码：在此计算机上更快（约 { $cpu } 毫秒，硬件解码器需 { $hw } 毫秒）
open-report-sw-scaled = 软件解码：硬件解码器只能以完整尺寸解码图像

## Hints
open-report-hint-copy-local = 将文件复制到本地磁盘可更快打开。
open-report-hint-extract = 将归档解压到文件夹可更快打开其中的页面。
open-report-hint-try-again = 要重新尝试硬件解码器：设置 → 性能 → 重试。
open-report-hint-automatic = 在“设置 → 性能”中将“图像处理位置”设为“自动”。
open-report-hint-turn-on = 在“设置 → 性能”中将“硬件解码”设为“自动”。
open-report-hint-free-memory = 关闭其他程序以释放内存。
open-report-hint-once-only = 只有第一张图像会这样检查。

## Rows
open-report-total = 直到清晰
open-report-first-pixels = 首个画面
# machine-drafted, needs native review
open-report-whole = 完整图像已显示
open-report-stage-startup = 启动
open-report-stage-read = 读取文件
open-report-stage-parse = 读取文件头
open-report-stage-colour = 准备色彩管理
open-report-stage-decode = 解码
open-report-stage-upload = 发送到 GPU
open-report-stage-waiting = 等待其他工作
open-report-decoder = 解码方式
open-report-decoder-checked = { $engine }，已与 CPU 比对
open-report-decoder-cache = 显示缓存
# $scale is the reduction: 4 for a level a quarter of the full width and height.
open-report-decoder-scaled = { $engine }，1/{ $scale } 尺寸
open-report-decoder-scaled-full = { $engine }，以完整尺寸解码后缩小到 1/{ $scale }
open-report-decoder-scaled-no-index = { $engine }，1/{ $scale } 尺寸，未使用快速启动索引
open-report-colour-conversion = 色彩转换方式

## Advisor: a pattern across recent opens, what would help and by how much (rows)
# $count and $total are counts of opens, $format a file format ("HEIC"), $speed a formatted
# number (MB/s), $low and $high formatted numbers ("2.5").
open-report-advice-recurring = 反复出现
open-report-advice-change = 改进方法
open-report-advice-gain = 预计提升
open-report-advice-software = 最近 { $total } 张较慢的 { $format } 图像中，有 { $count } 张因此原因使用软件解码
open-report-advice-slow-storage = 最近 { $total } 张较慢的图像中，有 { $count } 张耽搁在读取文件上（约 { $speed } MB/s）
# machine-drafted, needs native review
open-report-advice-slow-storage-no-speed = 最近 { $total } 张较慢的图像中，有 { $count } 张耽搁在读取文件上（小文件：主要在等待存储响应）
open-report-advice-archive = 最近 { $total } 张较慢的图像中，有 { $count } 张耽搁在解压归档上
open-report-advice-hardware-decoder = 带 { $codec } 硬件解码器的 GPU
open-report-advice-gain-range = 约快 { $low }–{ $high } 倍
open-report-advice-gain-one = 约快 { $low } 倍

## Performance insights: the indicator in the corner and the panel it opens
# machine-drafted, needs native review
# $total is a formatted duration ("240 ms"), $format a file format ("HEIC").
open-report-insights-title = 性能提示
open-report-insights-indicator = 为什么这么慢？
open-report-insights-announce = 性能提示：图像因同一原因反复打开缓慢。打开“为什么这么慢？”查看改进方法。
open-report-insights-hide = 隐藏
open-report-insights-last = 上一张较慢的图像：{ $total } 后清晰
open-report-insights-now = 现在可以做什么
open-report-insights-hardware = 有帮助的硬件
open-report-insights-none = 没有反复出现的减速
open-report-insights-none-detail = 当多张图像因同一原因打开缓慢时，会显示提示。为找出这些原因，{ -app-name } 会在此计算机上保留一周的慢速打开记录；不会发送到任何地方。
open-report-insights-software = { $format } 图像在 CPU 上解码
open-report-insights-storage = 读取文件拖慢了图像
open-report-insights-archive = 解压归档拖慢了图像
open-report-insights-nothing-now = { -app-name } 的设置无法改变这一点。
open-report-insights-display-cache = 开启显示缓存：之前打开过的图像将从此计算机上的快速副本打开。
open-report-insights-prefetch = 预先准备更多相邻图像，以便在你查看时准备好接下来的图像。
open-report-insights-hw-ssd = 用于这些文件的高速内置驱动器（NVMe SSD）
open-report-insights-action-turn-on = 开启
open-report-insights-action-try-again = 重试
open-report-insights-action-automatic = 设为“自动”
open-report-insights-dismiss-storage = 不再显示存储相关提示
open-report-insights-dismiss-decoder = 不再显示解码相关提示
open-report-insights-off = 关闭提示
open-report-insights-on = 开启提示
open-report-insights-restore = 重新显示已隐藏的提示
open-report-insights-power = 此计算机正在节能（使用电池或处于节能模式），这会减慢 CPU 上的解码。接通电源后，图像可能打开得更快。

## Performance insights in the gallery: the same, about making thumbnails
# machine-drafted, needs native review
# $count and $total are counts of thumbnails, $total in -last-thumbnail a formatted duration
# ("240 ms"), $format a file format ("HEIC"), $speed a formatted number (MB/s).
open-report-insights-announce-thumbnails = 性能提示：缩略图因同一原因反复显示缓慢。打开“为什么这么慢？”查看改进方法。
open-report-insights-last-thumbnail = 上一张较慢的缩略图：生成用时 { $total }
open-report-insights-thumbnails-software = { $format } 图像的缩略图在 CPU 上解码
open-report-insights-thumbnails-storage = 读取文件拖慢了缩略图
open-report-insights-thumbnails-archive = 解压归档拖慢了缩略图
open-report-insights-thumbnail-cache = 无法在此计算机上保留缩略图（缩略图缓存不可用），因此每次显示该文件夹时都会重新生成。
open-report-advice-thumbnails-software = 最近 { $total } 张较慢的 { $format } 缩略图中，有 { $count } 张因此原因使用软件解码
open-report-advice-thumbnails-slow-storage = 最近 { $total } 张较慢的缩略图中，有 { $count } 张耽搁在读取文件上（约 { $speed } MB/s）
# machine-drafted, needs native review
open-report-advice-thumbnails-slow-storage-no-speed = 最近 { $total } 张较慢的缩略图中，有 { $count } 张耽搁在读取文件上（小文件：主要在等待存储响应）
open-report-advice-thumbnails-archive = 最近 { $total } 张较慢的缩略图中，有 { $count } 张耽搁在解压归档上
