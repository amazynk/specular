# machine-drafted, needs native review
# Remote control and events (Settings → Integration), and hooks: programs settings.toml
# names to run when something happens in the viewer.

settings-integration-ipc = 远程控制与事件
settings-integration-ipc-help = 这台电脑上以你的身份运行的其他程序，可以通过私有套接字（specular ipc）控制 { -app-name } 并获知其中发生的事。不会在网络上监听。

## The hooks panel under "Remote control and events".
hooks-panel-title = 钩子
hooks-panel-intro = 发生某事时由 settings.toml 运行的程序。每个程序首次运行前会询问一次。
hooks-panel-none = 没有钩子。在 settings.toml 中以 [[hooks]] 添加。
hooks-panel-keywords = 钩子 hooks 事件 自动化 脚本 程序
# $events: the event names, joined ("marked, exported").
hooks-panel-on = 事件：{ $events }
hooks-panel-untrusted = 首次运行前询问
hooks-panel-allowed = 已允许
hooks-panel-remove = 移除…
hooks-panel-unused = 未使用

## Asking before a hook's first run (and again when its program or arguments change).
# $name: the hook's name; $program: the program; $events: what sets it off (hook-event-*,
# joined with hook-or).
hook-consent-title = 在{ $events }时运行“{ $name }”（{ $program }）？
hook-consent-detail = 它会以照片的名称启动此程序：{ $args }
hook-consent-trust = 只允许你信任的程序。钩子更改后会再次询问。
hook-consent-allow = 允许
hook-consent-not-now = 暂不
hook-or = 或
hook-event-opened = 打开照片
hook-event-marked = 标记照片
hook-event-keywords = 关键词更改
hook-event-exported = 导出照片
hook-event-imported = 导入照片
hook-event-captured = 相机发送照片
hook-event-develop-saved = 保存冲印编辑
hook-event-any = 发生任何事

## Removing a hook (Settings › Integration › Hooks › Remove…).
hook-remove-title = 移除钩子“{ $name }”？
hook-remove-detail = 它会从 settings.toml 中删除。程序和你的文件仍保留在磁盘上。
hook-remove-ok = 移除
hook-removed = 已移除钩子“{ $name }”。
hook-settings-unwritable = 无法更改 settings.toml：{ $reason }

## A hook that did not work (a quiet note; the log has the details).
hook-failed = 钩子“{ $name }”未能运行：{ $reason }
hook-failed-exit = 以代码 { $code } 结束
hook-failed-timeout = 在 { $seconds } 秒后被停止
hook-failed-start = 无法启动（{ $reason }）
hook-dropped = 钩子“{ $name }”正忙：已跳过 { $count } 个事件。

## An export started by another program (specular ipc export): nobody was asked.
export-recipes-remote-exists = 有 { $count } 个将被替换的文件已存在：未导出任何内容
export-recipes-remote-skipped = { $file } 已存在
