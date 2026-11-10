# Specular — OMERO servers and the Image Data Resource (IDR), browsed read-only. 简体中文. machine-drafted, needs native review

palette-omero-servers = OMERO 服务器…

omero-title = OMERO 服务器
omero-servers-intro = 只有在您浏览服务器时才会列出其上的项目、筛选和图像。在此之前不会发送任何内容。
omero-servers = 服务器
omero-servers-empty = 尚无服务器。请用其 OMERO.web 地址添加一个。
omero-session-only = 密码仅保留到 Specular 关闭为止。
omero-add = 添加…
omero-edit = 编辑…
omero-remove = 移除
omero-close = 关闭
omero-browse = 浏览
omero-back = 返回
omero-save = 保存
omero-stop = 停止
omero-open = 打开
# $server a server's name.
omero-removed = 已移除 { $server }
# Shown with servers reached without encryption (http://).
omero-not-encrypted = 未加密
# A server browsed without signing in.
omero-public = 公开
# $user the user name a server is signed in with.
omero-signed-in-as = 以 { $user } 身份

# The page that adds or edits a server.
omero-add-title = 添加服务器
omero-edit-title = 服务器设置
omero-field-name = 名称
omero-hint-name = 可选
omero-field-url = 地址
omero-hint-url = https://omero.example.org
omero-field-user = 用户名
omero-hint-user = 公开数据请留空
omero-field-password = 密码
omero-hint-password-kept = 留空以保留已输入的密码
omero-password-session = 密码仅保留到 Specular 关闭为止，绝不写入磁盘。
omero-password-needed = 请输入密码：它仅保留到 Specular 关闭为止。
omero-http-warning = 此地址未加密：密码将以明文发送。
omero-url-invalid = 请输入以 https:// 或 http:// 开头的 OMERO.web 地址。
# $reason why the list could not be saved.
omero-save-failed = 无法保存服务器列表：{ $reason }

# The browse page.
omero-browse-title = 浏览
omero-top = 项目和筛选
omero-datasets = 数据集
omero-plates = 板
omero-wells = 孔
omero-fields = 视野
omero-images = 图像
omero-kind-project = 项目
omero-kind-dataset = 数据集
omero-kind-image = 图像
omero-kind-screen = 筛选
omero-kind-plate = 板
omero-kind-well = 孔
# $well a well's row and column, as B7.
omero-well = 孔 { $well }
omero-well-fields = { $count ->
   *[other] { $count } 个视野
}
omero-children = { $count ->
   *[other] { $count } 项
}
omero-load-more = 加载更多
omero-loading = 正在加载…
omero-empty = 此处为空
omero-stopped = 已停止
omero-found = { $count ->
   *[other] { $count } 项
}
# $reason why the image could not be opened.
omero-open-failed = 无法打开图像：{ $reason }
# The channel keys would hide the last channel shown.
omero-channel-last = 至少保留一个通道显示
# $server a server's name.
omero-session-ended = { $server } 上的会话已结束。请输入密码以继续。

# Errors.
# $server a server's name.
omero-error-offline = 无法连接 { $server }。
omero-error-sign-in = 此服务器需要登录。
omero-error-expired = 会话已结束。
omero-error-login = 用户名或密码被拒绝。
omero-error-forbidden = 您的帐户无权查看此内容。
omero-error-not-found = 在服务器上未找到（可能已被移除）。
# $code an HTTP status code.
omero-error-status = 服务器返回错误 { $code }。
# $reason what was wrong with the answer.
omero-error-malformed = 无法读取服务器的响应：{ $reason }
