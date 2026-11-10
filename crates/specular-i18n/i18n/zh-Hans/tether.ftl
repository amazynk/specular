# Specular — tethered capture: a camera on USB, its shots downloaded as they are taken. 简体中文. machine-drafted, needs native review

palette-tether = 联机拍摄…
palette-tether-capture = 拍照（联机）

tether-title = 联机拍摄
tether-close = 关闭联机拍摄
tether-default-folder = 联机拍摄

## Cameras
tether-none = 未找到相机。
tether-none-hint = 用 USB 线连接相机并开机。如果相机有 USB 模式，请选择 PTP 或电脑连接。
tether-look-again = 重新查找
tether-connect = 连接
tether-next-camera = 下一台相机
tether-connecting = 正在连接…
tether-connected = 已连接 { $camera }
tether-disconnect = 断开
tether-try-again = 重试
tether-gone = 相机已断开。
tether-not-connected = 未连接相机。请先打开联机拍摄。

## Why a camera can't be opened
tether-permission = 当前用户无权打开相机的 USB 设备 { $node }。
tether-permission-rule = 一条 udev 规则可以允许访问。请以管理员身份把这一行写入 { $file }：
tether-permission-reload = 然后运行“sudo udevadm control --reload”，并重新插上相机。
tether-copy-rule = 复制规则
tether-busy-other = 另一个程序正在使用相机。
tether-busy-named = 相机正被 { $programs } 使用。
tether-busy-hint = 关闭那个程序，或在文件管理器中卸载相机，然后重试。
tether-driver = 此相机的系统驱动程序不允许其他程序使用它。

## The camera
tether-battery = 电池 { PERCENT($percent) }
tether-card = { $card }：可用 { $free }
tether-card-shots = { $card }：可用 { $free }，还可拍 { $shots } 张
tether-no-card = 没有存储卡。
tether-no-settings = 相机未报告曝光设置。
tether-no-remote = 无法从这里触发此相机。用相机快门按钮拍摄的照片会被下载。
tether-capture = 拍摄
tether-step-down = { $setting }：降低（{ $value }）
tether-step-up = { $setting }：提高（{ $value }）

## Exposure settings
tether-prop-mode = 模式
tether-prop-shutter = 快门
tether-prop-aperture = 光圈
tether-prop-iso = ISO 感光度
tether-prop-bias = 曝光补偿
tether-prop-white-balance = 白平衡
tether-auto = 自动
tether-bulb = B 门
tether-camera-value = 相机设置 { $value }
tether-wb-manual = 手动
tether-wb-one-push = 一键测量
tether-wb-daylight = 日光
tether-wb-fluorescent = 荧光灯
tether-wb-tungsten = 钨丝灯
tether-wb-flash = 闪光灯
tether-mode-manual = 手动
tether-mode-program = 程序自动
tether-mode-aperture = 光圈优先
tether-mode-shutter = 快门优先
tether-mode-creative = 创意
tether-mode-action = 运动
tether-mode-portrait = 人像

## Where shots go
tether-folder = 文件夹
tether-names = 文件名
tether-names-camera = 相机的文件名
tether-arrival = Develop 风格
tether-choose-folder = 文件夹…
tether-choose-style = 风格…
tether-downloading = 正在下载… { PERCENT($percent) }
tether-saved = 已保存 { $name }
tether-saved-count = { $count ->
   *[other] 已保存 { $count } 张
}

## Failures
tether-busy = 相机正忙。请重试。
tether-card-full = 存储卡已满。
tether-set-refused = 相机未接受该设置。请检查模式拨盘。
tether-disk = 无法保存照片：{ $error }
tether-error = 相机错误：{ $error }

## Settings › Cull mode
settings-cull-tether-folder = 联机拍摄文件夹
settings-cull-tether-folder-help = 用线缆连接的相机拍摄的照片保存在哪里。
settings-cull-tether-folder-unset = 图片 › 联机拍摄
settings-cull-tether-rename = 联机拍摄的文件名
settings-cull-tether-rename-help = 新文件名，使用与导入中“重命名”相同的标记。留空则保留相机的文件名。
