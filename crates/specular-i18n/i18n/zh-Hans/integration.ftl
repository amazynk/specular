# Integration: open with, user commands, paste (Stage 2).
palette-open-with = 打开方式…
palette-paste = 粘贴
open-with-placeholder = 打开方式…
open-with-loading = 正在查找应用程序…
open-with-default = { $name }（默认）
open-with-choose-other = 选择其他应用程序…
open-with-choose-title = 选择应用程序
open-with-failed = 无法用 { $name } 打开：{ $reason }
command-missing = 找不到 { $program }
command-failed = 无法启动 { $name }：{ $reason }
command-exited = { $name } 出错退出：{ $status }
paste-nothing = 剪贴板中没有可打开的内容
paste-failed = 无法粘贴图像：{ $reason }
drag-failed = 无法拖动文件：{ $reason }
about-storage = 设置：{ $settings } · 缓存：{ $cache }
about-storage-portable = 便携模式：设置和缓存位于 { $dir }

## Scripting and recipes

palette-copy-view-command = 将视图复制为 Shell 命令
palette-copy-view-python = 将视图复制为 Python
toast-copied-view-command = 已复制命令
script-copy-no-image = 没有可描述的图像
recipe-confirm-title = 首次运行配方“{ $name }”？它将运行：{ $command }
recipe-confirm-run = 运行
recipe-no-file = 该配方需要图像文件
recipe-running = 正在运行 { $name }…
recipe-done = { $name }：{ $shapes ->
   *[other] 已添加 { $shapes } 个形状
}
recipe-failed = { $name } 失败：{ $reason }
recipe-timeout = { $seconds } 秒后已停止
recipe-no-output = { $name } 未写出 { $file }
