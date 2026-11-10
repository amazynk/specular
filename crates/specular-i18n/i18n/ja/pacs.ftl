# machine-drafted, needs native review

## Send to PACS (C-STORE, STOW-RS), DIMSE servers, teaching quiz and teaching case export.

## Servers: DIMSE fields and Verify.

dicomweb-field-kind = 種類
dicomweb-kind-web = DICOMweb
dicomweb-kind-dimse = DICOM (DIMSE)
dicomweb-field-host = ホスト
dicomweb-hint-host = pacs.example.org
dicomweb-field-port = ポート
dicomweb-hint-port = 104 または 11112
# The application entity title the server answers to.
dicomweb-field-ae = サーバーの AE title
dicomweb-hint-ae = PACS
# The application entity title Specular sends as.
dicomweb-field-our-ae = こちらの AE title
# machine-drafted, needs native review
dicomweb-field-tls = セキュア接続 (TLS)
dicomweb-tls-on = セキュア接続: オン
dicomweb-tls-off = セキュア接続: オフ
dicomweb-problem-host = サーバーのホスト名またはアドレスを入力してください。
dicomweb-problem-port = ポートは 1 から 65535 までの数値です。
dicomweb-problem-ae = AE titleは 1～16 文字で、バックスラッシュは使えません。
# machine-drafted, needs native review
dicomweb-dimse-note-open = DICOM (DIMSE) サーバーでは、検索、シリーズを開くこと、送信ができます。
# How series are fetched: C-GET (over our own connection) or C-MOVE (the server connects back).
# machine-drafted, needs native review
pacs-field-retrieve = 取得方法
# machine-drafted, needs native review
pacs-retrieve-get = C-GET
# machine-drafted, needs native review
pacs-retrieve-move = C-MOVE
# C-MOVE: the address of this computer and the port the server sends to.
# machine-drafted, needs native review
pacs-field-receive-address = 受信アドレス
# machine-drafted, needs native review
pacs-hint-receive-address = このコンピューターのアドレス (例：192.168.1.20)
# machine-drafted, needs native review
pacs-field-receive-port = 受信ポート
# machine-drafted, needs native review
pacs-hint-receive-port = 例：11113
# machine-drafted, needs native review
pacs-move-note = C-MOVE を使うには、サーバーにこちらの AE title { $ae } がこのアドレスとポートで登録されている必要があります。
    Specular はシリーズの受信中だけそこで待ち受けます。
# machine-drafted, needs native review
pacs-problem-receive-address = 受信に使うこのコンピューターのアドレス (例：192.168.1.20) を入力してください。
# machine-drafted, needs native review
pacs-problem-receive-port = 受信ポートを入力してください。C-MOVE には必要です。
dicomweb-verify = 確認
dicomweb-verifying = サーバーを確認しています…
dicomweb-verify-ok = { $server } が応答しました。

## The Studies browser and the palette.

studies-send = PACS に送信…
studies-send-description = このシリーズ、その検査、またはディスク上のすべてをサーバーに送信します。
palette-send-to-pacs = PACS に送信…
palette-send-to-pacs-help = 現在のシリーズまたは検査を PACS サーバーに送信します。

## The Send page.

pacs-title = PACS に送信
# Under the title: what is being sent.
pacs-summary = { $studies ->
   *[other] { $studies } 件の検査
} · { $files ->
   *[other] { $files } 個のファイル
}
pacs-what = 送信対象
pacs-scope-series = このシリーズ ({ $files ->
   *[other] { $files } 個のファイル
})
pacs-scope-study = この検査 ({ $files ->
   *[other] { $files } 個のファイル
})
pacs-scope-all = 読み込んだすべて ({ $studies ->
   *[other] { $studies } 件の検査
}、{ $files ->
   *[other] { $files } 個のファイル
})
pacs-anonymise = 匿名化
pacs-anonymise-note = 送信するコピーから患者情報を削除します。元のファイルは変更されません。
pacs-identified-note = ファイルは患者情報を含んだまま、そのまま送信されます。
# The list of servers to choose from.
pacs-to = 送信先
pacs-no-servers = サーバーはまだありません。DICOMweb サーバーで追加してください。
pacs-send = 送信…
pacs-go-back = 戻る
pacs-confirm-send = 送信
pacs-confirm = { $studies ->
   *[other] { $studies } 件の検査
} ({ $files ->
   *[other] { $files } 個のファイル
}) を { $server } に送信しますか?
pacs-busy = 送信がまだ実行中です。
pacs-started = 送信しています。
pacs-stopping = 停止しています…
pacs-sending = { $server } に送信中: { $done } / { $total }…
pacs-sent-all = { $total ->
   *[other] { $total } 個のファイルをすべて { $server } に送信しました。
}
pacs-sent-some = { $total } 個中 { $sent } 個のファイルを { $server } に送信しました。
pacs-stopped = { $total } 個中 { $sent } 個のファイルで停止しました。
pacs-stopped-because = { $total } 個中 { $sent } 個のファイルで停止しました: { $reason }
pacs-warnings = { $count ->
   *[other] { $count } 個のファイルが警告付きで保存されました。
}
# A file that was not sent: its file name and why.
pacs-failed-line = { $name }: { $reason }
pacs-failed-more = ほか { $count } 件。

## Why one file was not sent.

pacs-reason-unreadable = 読み込めませんでした: { $reason }
pacs-reason-not-dicom = DICOM ファイルではありません
pacs-reason-burned-in = 画像に患者情報が焼き込まれているため、匿名化して送信しません
pacs-reason-document = 匿名化できない文書です
pacs-reason-anonymise = 匿名化できませんでした: { $reason }
pacs-reason-not-accepted = サーバーは { $class } を受け付けません
pacs-reason-convert = サーバー用に変換できませんでした: { $reason }
pacs-reason-resources = サーバーの容量またはリソースが不足しています
pacs-reason-mismatch = データが種類と一致しないとサーバーが応答しました
pacs-reason-understand = サーバーが読み取れませんでした
pacs-reason-class = サーバーはこの種類のオブジェクトに対応していません
pacs-reason-not-authorised = サーバーが許可していません
pacs-reason-status = サーバーが拒否しました (ステータス { $code })
pacs-reason-refused = サーバーが拒否しました
pacs-reason-too-large = サーバーには大きすぎます
pacs-reason-http = サーバーが拒否しました (エラー { $status })

## Why sending stopped, or Verify failed.

pacs-fatal-network-off = ネットワークがオフになっています。
pacs-fatal-unknown-host = { $host } は既知のホスト名ではありません。
pacs-fatal-connect = { $host } のポート { $port } で応答がありません。
pacs-fatal-unreachable = { $host } に到達できません。
pacs-fatal-timeout = { $host } が時間内に応答しませんでした。
pacs-fatal-cancelled = 停止しました。
pacs-fatal-called-ae = { $host } は AE title { $ae } に応答しません。サーバーの AE titleを確認してください。
pacs-fatal-calling-ae = { $host } はこちらの AE title { $ae } を認識していません。サーバーに追加してください。
pacs-fatal-busy = { $host } はビジー状態です。後でもう一度お試しください。
pacs-fatal-rejected = { $host } が接続を拒否しました。
pacs-fatal-aborted = { $host } が接続を終了しました。
pacs-fatal-nothing-accepted = { $host } はこれらの種類のオブジェクトをどれも受け付けません。
pacs-fatal-protocol = { $host } の応答を Specular が理解できませんでした。
pacs-fatal-auth = { $host } がサインインを受け付けませんでした。
pacs-fatal-http = { $host } がエラー { $status } を返しました。

## Teaching quiz.

palette-teaching-quiz = 教育用クイズ
palette-teaching-quiz-help = キー画像または教育用症例を順に表示し、所見は表示するまで隠しておきます。
# "Case 2 of 5".
quiz-case = 症例 { $n } / { $total }
quiz-hint-hidden = スペースで所見を表示 · → 次の症例 · Esc で終了
quiz-hint-revealed = スペースで再び隠す · → 次の症例 · Esc で終了
quiz-hidden = 所見は隠れています。
quiz-no-findings = 所見は記録されていません。
quiz-notes = 症例ノート: { $notes }
quiz-ended = クイズを終了しました。
quiz-empty = 表示する症例がありません。K でキー画像に印を付けるか、教育用症例フォルダーを選んでください。
quiz-not-a-case = このフォルダーは Specular が書き出した教育用症例ではありません。
quiz-pick-title = 教育用症例フォルダーを選択

## Teaching case folder (Export key images).

figure-keys-case = 教育用症例フォルダー
figure-case-notes = 症例ノート
figure-case-findings = 所見
figure-case-none = 記録なし
figure-case-readme = Specular が書き出した教育用症例です。患者情報は削除されています。
    images: キー画像。焼き込み文字は黒塗りされています。
    DICOM: 匿名化したコピーと、Key Object Selection マニフェスト "For Teaching"。
    case.txt: 症例ノートと各画像の所見。case.json: スクリプト用の同じ内容。
    Radiopaedia の場合: DICOM フォルダーまたは画像をアップロードし、case.txt からノートをコピーしてください。

## Search on a DIMSE server (C-FIND).
# machine-drafted, needs native review
pacs-find-not-supported = { $host } は検索 (C-FIND) に応答しません。
pacs-find-status = { $host } が検索を拒否しました (ステータス { $code })。

## Opening a series from a DIMSE server (C-GET, C-MOVE).
# machine-drafted, needs native review
pacs-retrieve-no-get = { $host } は C-GET でシリーズを送信しません。
    サーバーの設定で C-MOVE を選んでください。
# machine-drafted, needs native review
pacs-retrieve-no-move = { $host } は C-MOVE 要求を受け付けません。
# machine-drafted, needs native review
pacs-retrieve-unknown-destination = { $host } はこちらの AE title { $ae } を認識していません。
    このコンピューターのアドレスとポート { $port } でサーバーに追加してください。
# machine-drafted, needs native review
pacs-retrieve-resources = { $host } のリソースが不足しています。後でもう一度お試しください。
# machine-drafted, needs native review
pacs-retrieve-status = { $host } がシリーズの送信を拒否しました (ステータス { $code })。
# machine-drafted, needs native review
pacs-retrieve-no-matches = { $host } にはこのシリーズの画像がありません。
# machine-drafted, needs native review
pacs-retrieve-roles = { $host } がこの接続での画像の送信を拒否しました。
    サーバーの設定で C-MOVE を選んでください。
# machine-drafted, needs native review
pacs-retrieve-move-nothing = { $host } はこのコンピューターに画像を送信できませんでした。
    アドレス、ポート、ファイアウォールと、サーバーがこちらの AE title { $ae } を認識しているかを確認してください。
# machine-drafted, needs native review
pacs-retrieve-nothing = { $host } は{ $count ->
   *[other] { $count } 枚の画像のいずれも
} Specular が読める形式で送信できませんでした。
# machine-drafted, needs native review
pacs-retrieve-listen = { $address } のポート { $port } で受信できません：{ $reason }

## DICOM over TLS.
# machine-drafted, needs native review
pacs-fatal-tls-certificate = { $host } の証明書は信頼されていません。
    発行元がシステムの信頼された証明書に含まれている必要があります。
pacs-fatal-tls = { $host } とのセキュア接続がありません。
    ポート { $port } が DICOM over TLS に対応しているか確認してください。
