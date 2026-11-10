# Specular — DICOMweb servers, study search and streamed series (DESIGN.md §12.8). 简体中文.

palette-dicom-servers = DICOMweb 服务器…

dicomweb-title = DICOMweb 服务器
dicomweb-servers-intro = 只有在您请求时才会搜索和打开服务器上的检查。在此之前不会发送任何内容。
dicomweb-servers = 服务器
dicomweb-servers-empty = 尚无服务器。请用其 DICOMweb 地址添加一个。
dicomweb-session-only = 密码和令牌仅保留到 Specular 关闭为止。
dicomweb-add = 添加…
dicomweb-edit = 编辑…
dicomweb-remove = 移除
dicomweb-close = 关闭
dicomweb-browse = 浏览
dicomweb-back = 返回
dicomweb-save = 保存
dicomweb-search = 搜索
dicomweb-stop = 停止
dicomweb-removed = 已移除 { $server }
# machine-drafted, needs native review
dicomweb-remove-ask = 要从已保存的服务器中移除“{ $server }”吗？
dicomweb-remove-detail = 磁盘上的内容不会丢失。其本次会话的密码或令牌将被清除。
# 显示于未加密 (http://) 连接的服务器。
dicomweb-not-encrypted = 未加密

# 添加或编辑服务器的页面。
dicomweb-add-title = 添加服务器
dicomweb-edit-title = 服务器设置
dicomweb-field-name = 名称
dicomweb-hint-name = 可选
dicomweb-field-url = 地址
dicomweb-hint-url = https://pacs.example.org/dicom-web
dicomweb-field-auth = 登录
dicomweb-auth-none = 无需登录
dicomweb-auth-basic = 用户名和密码
dicomweb-auth-bearer = 访问令牌
dicomweb-field-user = 用户名
dicomweb-field-password = 密码
dicomweb-field-token = 访问令牌
dicomweb-hint-secret-kept = 留空则保留已输入的值
dicomweb-secret-session = 密码或令牌仅保留到 Specular 关闭为止，绝不写入磁盘。
dicomweb-secret-needed = 请重新输入密码或令牌：它仅保留到 Specular 关闭为止。
dicomweb-http-warning = 此地址未加密：密码或令牌将以明文发送。
dicomweb-url-empty = 请输入服务器的 DICOMweb 地址。
dicomweb-url-scheme = 地址必须以 https:// 或 http:// 开头。
dicomweb-url-host = 地址中没有服务器名称。
dicomweb-url-userinfo = 请将用户名和密码填入登录字段，而不是地址中。
dicomweb-save-failed = 无法保存服务器列表：{ $reason }

# 搜索页面。
dicomweb-search-title = 查找检查
dicomweb-field-patient-name = 患者姓名
dicomweb-field-patient-id = 患者 ID
dicomweb-field-date-from = 从
dicomweb-field-date-to = 至
dicomweb-hint-date = YYYYMMDD
dicomweb-field-modality = 模态
dicomweb-hint-modality = CT, MR…
dicomweb-field-accession = 检查号
dicomweb-show-patient = 显示患者数据
dicomweb-switch-on = 开
dicomweb-switch-off = 关
dicomweb-patient-shown = 已显示患者数据
dicomweb-patient-hidden = 已隐藏患者数据
dicomweb-studies = 检查
dicomweb-search-hint = 填写任意字段后搜索。
dicomweb-no-studies = 没有匹配的检查。
dicomweb-searching = 正在搜索…
dicomweb-stopped = 已停止
dicomweb-load-more = 加载更多
dicomweb-study-untitled = 无描述的检查
dicomweb-series-untitled = 无描述的序列
dicomweb-study-counts = { $series } 个序列，{ $images } 张图像
dicomweb-series-images = { $count } 张图像
dicomweb-patient-id = ID { $id }
dicomweb-accession = 检查号 { $number }
dicomweb-found = 找到 { $count } 个检查
dicomweb-series-found = { $count } 个序列

# 与服务器通信时的错误。
dicomweb-error-offline = 无法连接 { $server }。请检查网络和地址。
dicomweb-error-auth = 服务器拒绝了用户名、密码或令牌。
dicomweb-error-not-found = 此地址没有 DICOMweb 服务。
dicomweb-error-other = 服务器无法响应：{ $reason }

# 流式传输到查看器的序列。
dicomweb-loading = 正在加载 { $series }…
dicomweb-loading-progress = 正在加载 { $series }：{ $done } / { $total }
dicomweb-loaded = { $series }：已加载 { $total } 张图像
dicomweb-loaded-failed = { $total } 张图像中有 { $failed } 张无法加载。
dicomweb-offline-cached = 无法连接 { $server }：正在显示之前保存的图像。
dicomweb-cache-failed = 无法打开下载文件夹：{ $reason }
