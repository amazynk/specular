# machine-drafted, needs native review
## 手册面板、崩溃报告、诊断包、更新检查和“关于”。简体中文。
## 风格：简短、清楚、不加赘述；一句一个意思。

## 面板

palette-help-manual = 帮助
palette-help-manual-help = 打开手册中与当前屏幕内容相关的页面。
palette-save-diagnostics = 保存诊断包…
palette-save-diagnostics-help = 将崩溃报告、日志和系统详情保存为 zip 文件。不会发送任何内容。
# 打开手册某一节的命令面板条目；{ $title } 是页面或“页面 › 小节”。
palette-help-topic = 帮助：{ $title }
palette-help-topic-help = 打开手册的这一节。
# 显示在命令面板中不可用的命令下方。
palette-disabled-help = 现在不可用。按 Enter 查看原因。

## 帮助面板

help-title = 帮助
help-search-placeholder = 搜索手册
help-contents = 目录
help-back = 后退
help-forward = 前进
help-no-results = 未找到与“{ $query }”相关的内容。
help-results = { $count ->
   *[other] 找到 { $count } 个小节
}
help-section-start = 入门
help-section-modes = 查看与模式
help-section-tasks = 如何…
help-section-reference = 参考
help-keys-unbound = 未设置
# 手册中提到但没有按键的命令；从命令面板运行。
help-key-unbound = { $name }（命令面板）
help-keys-command = 命令
help-keys-keys = 按键
help-commands = 所有命令都在命令面板中（{ $key }）。输入名称的一部分即可。
help-commands-no-key = 所有命令都在命令面板中。输入名称的一部分即可。
help-link = 转到 { $title }
help-open = 帮助
# 面板的屏幕阅读器名称；{ $page } 是当前页面。
help-label = 帮助：{ $page }
help-disabled-why = 为什么？

## 下次启动时提供的崩溃报告

crash-title = { -app-name } 意外关闭
crash-body = 报告已保存在这台电脑上。未发送任何内容。
crash-view = 查看报告
crash-hide = 隐藏报告
crash-copy = 复制
crash-copied = 报告已复制
crash-save-bundle = 保存诊断包…
crash-dismiss = 关闭

## 诊断包

diag-title = 保存诊断包
diag-intro = 用于附加到错误报告的 zip 文件。它保存在这台电脑上。不会发送任何内容。
diag-contents = 内容
diag-include-names = 包含文件名
diag-include-names-help = 关闭：名称替换为 file-1.jpg、dir-1 等。
diag-privacy = 你的主文件夹显示为 ~，用户名显示为 <user>。绝不包含密码、服务器地址和图像。
diag-where = 报告问题：{ $issues } · { $email }
diag-save = 保存…
diag-cancel = 取消
diag-saved = 诊断包已保存：{ $path }
diag-failed = 无法保存诊断包：{ $reason }
diag-entry-summary = 版本、系统、GPU 和驱动程序
diag-entry-crash = 最近的崩溃报告
diag-entry-crashes = 更早的崩溃报告（{ $count }）
diag-entry-log = 日志最后 { $count } 行
diag-entry-settings = 设置（不含密码和地址）
diag-entry-gpu = GPU 和显示器详情
diag-entry-readme = 这个包是什么
diag-size = { $size }
# 包内的 README.txt。
diag-readme =
    { -app-name } 诊断包。

    其中包含有助于查找问题的内容：版本和系统详情、崩溃报告、日志末尾和设置。

    你的主文件夹显示为 ~，用户名显示为 <user>。除非你选择包含，文件名和文件夹名会替换为 file-1、dir-1 等。密码和服务器地址已删除。

    未发送任何内容。是否分享此文件由你决定。

## 更新（设置）

settings-section-updates = 更新
settings-updates-check = 检查更新
settings-updates-check-help = 每天一次，询问是否有新版本。请求只包含版本号：没有标识符，没有 Cookie。关闭：{ -app-name } 绝不会为更新而联网。
settings-updates-channel = 通道
settings-updates-channel-help = 稳定版：仅正式发布版。测试版：也包括测试版本。
settings-updates-channel-stable = 稳定版
settings-updates-channel-beta = 测试版
settings-updates-check-now = 立即检查
settings-updates-check-now-help = 现在检查一次。
update-available = 版本 { $version } 可用
update-notes = 发行说明
update-download = 下载
update-none = { -app-name } 已是最新版本。
update-checking = 正在检查更新…
update-failed = 无法检查更新：{ $reason }
update-last-checked = 上次检查：{ $when }

## 关于

about-build = 构建 { $commit } · { $profile } · { $target }
about-build-no-commit = 构建 { $profile } · { $target }
about-help = 帮助
about-diagnostics = 保存诊断包…
# machine-drafted, needs native review
about-renderer = 渲染器信息…
about-licences-count = { $count ->
   *[other] { $count } 个第三方组件
}
about-licence-group = { $licence }（{ $count }）
about-contact = 问题反馈：{ $issues } · 支持：{ $email }
