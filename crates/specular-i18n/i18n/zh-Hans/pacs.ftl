# machine-drafted, needs native review

## Send to PACS (C-STORE, STOW-RS), DIMSE servers, teaching quiz and teaching case export.

## Servers: DIMSE fields and Verify.

dicomweb-field-kind = 类型
dicomweb-kind-web = DICOMweb
dicomweb-kind-dimse = DICOM (DIMSE)
dicomweb-field-host = 主机
dicomweb-hint-host = pacs.example.org
dicomweb-field-port = 端口
dicomweb-hint-port = 104 或 11112
# The application entity title the server answers to.
dicomweb-field-ae = 服务器 AE title
dicomweb-hint-ae = PACS
# The application entity title Specular sends as.
dicomweb-field-our-ae = 本机 AE title
# machine-drafted, needs native review
dicomweb-field-tls = 安全连接 (TLS)
dicomweb-tls-on = 安全连接已开启
dicomweb-tls-off = 安全连接已关闭
dicomweb-problem-host = 请输入服务器的主机名或地址。
dicomweb-problem-port = 端口是 1 到 65535 之间的数字。
dicomweb-problem-ae = AE title 为 1 到 16 个字符，不含反斜杠。
# machine-drafted, needs native review
dicomweb-dimse-note-open = 可以在 DICOM (DIMSE) 服务器中搜索、打开序列，也可以向其发送。
# How series are fetched: C-GET (over our own connection) or C-MOVE (the server connects back).
# machine-drafted, needs native review
pacs-field-retrieve = 获取方式
# machine-drafted, needs native review
pacs-retrieve-get = C-GET
# machine-drafted, needs native review
pacs-retrieve-move = C-MOVE
# C-MOVE: the address of this computer and the port the server sends to.
# machine-drafted, needs native review
pacs-field-receive-address = 接收地址
# machine-drafted, needs native review
pacs-hint-receive-address = 本机地址，例如 192.168.1.20
# machine-drafted, needs native review
pacs-field-receive-port = 接收端口
# machine-drafted, needs native review
pacs-hint-receive-port = 例如 11113
# machine-drafted, needs native review
pacs-move-note = 使用 C-MOVE 时，服务器必须以此地址和端口登记我们的 AE title { $ae }。
    Specular 仅在接收序列期间在此监听。
# machine-drafted, needs native review
pacs-problem-receive-address = 请输入用于接收的本机地址（例如 192.168.1.20）。
# machine-drafted, needs native review
pacs-problem-receive-port = 请输入用于接收的端口。C-MOVE 需要端口。
dicomweb-verify = 验证
dicomweb-verifying = 正在检查服务器…
dicomweb-verify-ok = { $server } 已响应。

## The Studies browser and the palette.

studies-send = 发送到 PACS…
studies-send-description = 将此序列、其所属检查或光盘上的全部内容发送到服务器。
palette-send-to-pacs = 发送到 PACS…
palette-send-to-pacs-help = 将当前序列或检查发送到 PACS 服务器。

## The Send page.

pacs-title = 发送到 PACS
# Under the title: what is being sent.
pacs-summary = { $studies ->
   *[other] { $studies } 项检查
} · { $files ->
   *[other] { $files } 个文件
}
pacs-what = 发送内容
pacs-scope-series = 此序列（{ $files ->
   *[other] { $files } 个文件
}）
pacs-scope-study = 此检查（{ $files ->
   *[other] { $files } 个文件
}）
pacs-scope-all = 已读取的全部内容（{ $studies ->
   *[other] { $studies } 项检查
}，{ $files ->
   *[other] { $files } 个文件
}）
pacs-anonymise = 匿名化
pacs-anonymise-note = 发送的副本会去除患者信息。您的文件不会改变。
pacs-identified-note = 文件按原样发送，包含患者信息。
# The list of servers to choose from.
pacs-to = 发送到
pacs-no-servers = 尚无服务器。请在“DICOMweb 服务器”中添加。
pacs-send = 发送…
pacs-go-back = 返回
pacs-confirm-send = 发送
pacs-confirm = 将 { $studies ->
   *[other] { $studies } 项检查
}（{ $files ->
   *[other] { $files } 个文件
}）发送到 { $server }？
pacs-busy = 仍有发送任务在进行。
pacs-started = 正在发送。
pacs-stopping = 正在停止…
pacs-sending = 正在向 { $server } 发送 { $done } / { $total }…
pacs-sent-all = { $total ->
   *[other] 全部 { $total } 个文件已发送到 { $server }。
}
pacs-sent-some = { $total } 个文件中有 { $sent } 个已发送到 { $server }。
pacs-stopped = 在 { $total } 个文件中发送了 { $sent } 个后停止。
pacs-stopped-because = 在 { $total } 个文件中发送了 { $sent } 个后停止：{ $reason }
pacs-warnings = { $count ->
   *[other] { $count } 个文件存储时带有警告。
}
# A file that was not sent: its file name and why.
pacs-failed-line = { $name }：{ $reason }
pacs-failed-more = 另有 { $count } 个。

## Why one file was not sent.

pacs-reason-unreadable = 无法读取：{ $reason }
pacs-reason-not-dicom = 不是 DICOM 文件
pacs-reason-burned-in = 图像中内嵌了患者信息，因此不以匿名方式发送
pacs-reason-document = 无法匿名化的文档
pacs-reason-anonymise = 无法匿名化：{ $reason }
pacs-reason-not-accepted = 服务器不接收 { $class }
pacs-reason-convert = 无法为服务器转换：{ $reason }
pacs-reason-resources = 服务器空间或资源不足
pacs-reason-mismatch = 服务器称数据与其类型不符
pacs-reason-understand = 服务器无法读取
pacs-reason-class = 服务器不支持此类对象
pacs-reason-not-authorised = 服务器不允许
pacs-reason-status = 服务器拒绝了它（状态 { $code }）
pacs-reason-refused = 服务器拒绝了它
pacs-reason-too-large = 对服务器来说太大
pacs-reason-http = 服务器拒绝了它（错误 { $status }）

## Why sending stopped, or Verify failed.

pacs-fatal-network-off = 网络已关闭。
pacs-fatal-unknown-host = { $host } 不是已知的主机名。
pacs-fatal-connect = { $host } 的端口 { $port } 无响应。
pacs-fatal-unreachable = 无法访问 { $host }。
pacs-fatal-timeout = { $host } 未及时响应。
pacs-fatal-cancelled = 已停止。
pacs-fatal-called-ae = { $host } 不响应 AE title { $ae }。请检查服务器的 AE title。
pacs-fatal-calling-ae = { $host } 不认识我们的 AE title { $ae }。请在服务器上添加。
pacs-fatal-busy = { $host } 正忙。请稍后重试。
pacs-fatal-rejected = { $host } 拒绝了连接。
pacs-fatal-aborted = { $host } 终止了连接。
pacs-fatal-nothing-accepted = { $host } 不接收这些对象类型中的任何一种。
pacs-fatal-protocol = { $host } 的响应方式 Specular 无法理解。
pacs-fatal-auth = { $host } 未接受登录。
pacs-fatal-http = { $host } 返回错误 { $status }。

## Teaching quiz.

palette-teaching-quiz = 教学测验
palette-teaching-quiz-help = 逐个浏览关键图像或教学病例，所见在您显示之前保持隐藏。
# "Case 2 of 5".
quiz-case = 病例 { $n } / { $total }
quiz-hint-hidden = 空格键显示所见 · → 下一病例 · Esc 结束
quiz-hint-revealed = 空格键再次隐藏 · → 下一病例 · Esc 结束
quiz-hidden = 所见已隐藏。
quiz-no-findings = 未记录所见。
quiz-notes = 病例备注：{ $notes }
quiz-ended = 测验已结束。
quiz-empty = 没有可显示的病例。请用 K 标记关键图像，或选择一个教学病例文件夹。
quiz-not-a-case = 此文件夹不是由 Specular 导出的教学病例。
quiz-pick-title = 选择教学病例文件夹

## Teaching case folder (Export key images).

figure-keys-case = 教学病例文件夹
figure-case-notes = 病例备注
figure-case-findings = 所见
figure-case-none = 未记录
figure-case-readme = 由 Specular 导出的教学病例。患者信息已去除。
    images：关键图像，内嵌文字已涂黑。
    DICOM：匿名化副本和一个 Key Object Selection 清单“For Teaching”。
    case.txt：病例备注和每张图像的所见。case.json：供脚本使用的相同内容。
    用于 Radiopaedia：上传 DICOM 文件夹或图像，并从 case.txt 复制备注。

## Search on a DIMSE server (C-FIND).
# machine-drafted, needs native review
pacs-find-not-supported = { $host } 不响应搜索 (C-FIND)。
pacs-find-status = { $host } 拒绝了搜索 (状态 { $code })。

## Opening a series from a DIMSE server (C-GET, C-MOVE).
# machine-drafted, needs native review
pacs-retrieve-no-get = { $host } 不通过 C-GET 发送序列。
    请在服务器设置中选择 C-MOVE。
# machine-drafted, needs native review
pacs-retrieve-no-move = { $host } 不接受 C-MOVE 请求。
# machine-drafted, needs native review
pacs-retrieve-unknown-destination = { $host } 不认识我们的 AE title { $ae }。
    请在服务器上用本机地址和端口 { $port } 添加它。
# machine-drafted, needs native review
pacs-retrieve-resources = { $host } 资源不足。请稍后重试。
# machine-drafted, needs native review
pacs-retrieve-status = { $host } 拒绝发送该序列 (状态 { $code })。
# machine-drafted, needs native review
pacs-retrieve-no-matches = { $host } 没有此序列的图像。
# machine-drafted, needs native review
pacs-retrieve-roles = { $host } 拒绝通过此连接发送图像。
    请在服务器设置中选择 C-MOVE。
# machine-drafted, needs native review
pacs-retrieve-move-nothing = { $host } 无法将图像发送到本机。
    请检查地址、端口和防火墙，以及服务器是否认识我们的 AE title { $ae }。
# machine-drafted, needs native review
pacs-retrieve-nothing = { $host } 无法以 Specular 可读的格式发送{ $count ->
   *[other] { $count } 张图像中的任何一张
}。
# machine-drafted, needs native review
pacs-retrieve-listen = 无法在 { $address } 的端口 { $port } 上接收：{ $reason }

## DICOM over TLS.
# machine-drafted, needs native review
pacs-fatal-tls-certificate = { $host } 的证书不受信任。
    其颁发者必须在系统受信任的证书中。
pacs-fatal-tls = 无法与 { $host } 建立安全连接。
    请检查端口 { $port } 是否使用基于 TLS 的 DICOM。
