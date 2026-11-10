# machine-drafted, needs native review
# ファイル操作：名前の変更、一括名前変更、フォルダーへのコピーと移動、ファイルに保存する回転、
# 取り消しとやり直し、壁紙に設定、共有、メールで送信、設定の書き出しと読み込み。

## パレット、チートシート、メニュー

palette-undo = 取り消す
palette-undo-help = ファイルへの最後の変更を取り消します：ゴミ箱への移動、名前の変更、コピーや移動、回転。
palette-redo = やり直す
palette-redo-help = 最後に取り消した操作をもう一度行います。
palette-rename = 名前を変更…
palette-rename-help = ファイルに新しい名前を付けます。サイドカーと RAW+JPEG の相方も一緒に変わります。
palette-batch-rename = 複数のファイル名を変更…
palette-batch-rename-help = 選択したファイル、またはフォルダー全体の名前をパターンから変更します。プレビュー付き。
palette-copy-to-folder = フォルダーにコピー…
palette-copy-to-folder-help = ファイルまたは選択項目を別のフォルダーにコピーします。
palette-move-to-folder = フォルダーに移動…
palette-move-to-folder-help = ファイルまたは選択項目を別のフォルダーに移動します。
palette-rotate-file-left = ファイルを左に回転
palette-rotate-file-left-help = ファイルそのものを左に 90° 回転します。形式が許す場合は劣化なしで。
palette-rotate-file-right = ファイルを右に回転
palette-rotate-file-right-help = ファイルそのものを右に 90° 回転します。形式が許す場合は劣化なしで。
palette-set-wallpaper-help = この画像をデスクトップの背景にします。
palette-share = 共有…
palette-share-help = システムの共有メニューでファイルを別のアプリに送ります。
palette-send-by-email = メールで送信…
palette-send-by-email-help = ファイルを添付した新しいメールをメールアプリで作成します。
# machine-drafted, needs native review
palette-share-with-zynk = Zynkで共有…
palette-share-with-zynk-help = Zynkアプリでファイルのウェブリンクを作成します。
palette-send-with-zynk = Zynkで送信…
palette-send-with-zynk-help = Zynkで連絡先または自分のデバイスにファイルを送ります。
palette-export-settings = 設定を書き出す…
palette-export-settings-help = すべての設定とキー割り当てをファイルに保存します。
palette-import-settings = 設定を読み込む…
palette-import-settings-help = 変更内容を表示してから、ファイルから設定を読み込みます。

disabled-in-archive-file = このページはアーカイブ内にあり、独自のファイルがありません
disabled-not-rotatable = この種類のファイルは回転を記録できません
disabled-not-on-this-desktop = お使いのデスクトップはこの機能に対応していません

## 設定

settings-browsing-rotate-trim-edges = JPEG の端を切り詰めて劣化なしで回転
settings-browsing-rotate-trim-edges-help = 幅や高さが 8 または 16 ピクセルの倍数でない JPEG は、最後の数ピクセルを切り落とさないと劣化なしで回転できません。オフにすると代わりに向きのタグを変更し、毎回切り詰めるかを尋ねます。
settings-browsing-rotate-keep-modified = 回転しても更新日時を保つ
settings-browsing-rotate-keep-modified-help = 回転したファイルは最終更新日時を保つので、日付順に並べても位置が変わりません。
settings-browsing-recent-folders = 最近使ったフォルダー
settings-browsing-recent-folders-help = コピーと移動で最近使ったフォルダー。

## シート

fileop-cancel = キャンセル
rename-title = 名前の変更
rename-field-label = 名前
rename-button = 名前を変更
rename-sidecars-follow = 一緒に名前を変更：{ $files }
rename-extension-changed = 拡張子を変えてもファイルは変換されません。他のアプリで開けなくなることがあります。
rename-problem-empty = 名前を入力してください。
rename-problem-invalid-char = 名前に「{ $char }」は使えません。
rename-problem-reserved = 「{ $name }」は Windows がデバイス用に予約している名前です。
rename-problem-too-long = 名前が長すぎます。255 バイトまでです。
rename-problem-dot = 「.」と「..」は名前に使えません。
rename-problem-trailing = 名前の末尾にスペースやピリオドは使えません。
rename-problem-exists = この名前のファイルがすでにあります。
rename-problem-read-only = このフォルダーは変更できません。
rename-problem-duplicate = 2 つのファイルが同じ名前になります。

batch-rename-title =
    { $count ->
       *[other] { $count } 個のファイル名を変更
    }
batch-rename-field-label = パターン
batch-rename-tokens = { "{" }name{ "}" }、{ "{" }n:3{ "}" }（001、002、…）、{ "{" }date:%Y-%m-%d{ "}" }、{ "{" }camera{ "}" }、{ "{" }w{ "}" }×{ "{" }h{ "}" }、{ "{" }ext{ "}" } を使えます。
batch-rename-summary =
    { $count ->
       *[other] { $count } 個のファイルの名前を変更します
    }{ $problems ->
        [0] 。
       *[other] 。{ $problems } 個は変更できないため、そのままです。
    }
batch-rename-reading = 日付とカメラを読み取り中…
batch-rename-preview = プレビュー
batch-rename-unchanged = 名前はそのまま
batch-rename-more =
    { $count ->
       *[other] 下にあと { $count } 個
    }

transfer-copy-title-one = 「{ $file }」をフォルダーにコピー
transfer-copy-title =
    { $count ->
       *[other] { $count } 個のファイルをフォルダーにコピー
    }
transfer-move-title-one = 「{ $file }」をフォルダーに移動
transfer-move-title =
    { $count ->
       *[other] { $count } 個のファイルをフォルダーに移動
    }
transfer-recent = 最近使ったフォルダー
transfer-folder = フォルダー
transfer-choose-folder = フォルダーを選択…
transfer-copy-dialog = フォルダーにコピー
transfer-move-dialog = フォルダーに移動
transfer-sidecars =
    { $count ->
       *[other] サイドカーファイル { $count } 個も一緒に処理します。
    }
transfer-taken =
    { $count ->
       *[other] { $folder } で { $count } 個の名前がすでに使われています。
    }
transfer-when-taken = 名前がすでに使われているとき
transfer-skip = スキップ
transfer-keep-both = 両方残す
transfer-replace-option = 置き換える
transfer-copy = コピー
transfer-move = 移動
transfer-replace-title =
    { $count ->
       *[other] { $folder } の { $count } 個のファイルを置き換えますか？
    }
transfer-replace-note = 既存のファイルはゴミ箱に移動します。取り消せば元に戻ります。
transfer-replace = 置き換える
transfer-back = 戻る
transfer-stop = 停止

settings-file-import-title = 設定の読み込み
settings-file-import-dialog = 設定の読み込み
settings-file-export-dialog = 設定の書き出し
settings-file-import-changes = 変更点
settings-file-import-change = { $before } → { $after }
settings-file-import-no-key = キーなし
settings-file-import-unknown = このバージョンにない設定のため除外します。
settings-file-import-keymap = キー割り当て
settings-file-import-nothing = 変更はありません。これらの設定はすでに有効です。
settings-file-import-summary =
    { $count ->
       *[other] 変更 { $count } 件。読み込みは設定画面で取り消せます。
    }
settings-file-import-secrets =
    { $count ->
       *[other] パスワードやキーと思われる { $count } 件の項目は読み込みません。
    }
settings-file-import-commands =
    { $count ->
       *[other] ユーザーコマンド { $count } 件は読み込みません。コマンドはプログラムを起動するためです。信頼できる場合は自分で設定ファイルにコピーしてください。
    }
settings-file-import-button = 読み込む

## 結果

fileop-renamed = { $file } に名前を変更しました
fileop-renamed-count =
    { $count ->
       *[other] { $count } 個のファイル名を変更しました
    }
fileop-copied =
    { $count ->
       *[other] { $count } 個のファイルを { $folder } にコピーしました
    }
fileop-moved =
    { $count ->
       *[other] { $count } 個のファイルを { $folder } に移動しました
    }
fileop-skipped =
    { $count ->
       *[other] { $count } 個をスキップしました：移動先で名前がすでに使われています。
    }
fileop-stopped = 停止しました。済んだ分はそのままです。
fileop-failed-file =
    { $count ->
       *[other] { $count } 個のファイルをコピーまたは移動できませんでした。最初は { $file }：{ $reason }
    }
fileop-progress-copy =
    { $count ->
       *[other] { $count } 個のファイルをコピー中… { PERCENT($percent) }
    }
fileop-progress-move =
    { $count ->
       *[other] { $count } 個のファイルを移動中… { PERCENT($percent) }
    }
fileop-done-status = 完了
fileop-busy = 前のファイル操作を処理中です。少し待ってからもう一度お試しください。
fileop-nothing-to-redo = やり直す操作はありません
fileop-undone = 取り消しました：{ $what }
fileop-redone = やり直しました：{ $what }
fileop-kind-rename = 名前の変更
fileop-kind-batch-rename = 複数ファイルの名前変更
fileop-kind-move = フォルダーへの移動
fileop-kind-copy = フォルダーへのコピー
fileop-kind-rotate = 回転
rename-failed = 名前を変更できませんでした：{ $reason }

undo-problem-changed = 取り消せません：{ $file } はその後変更されており、取り消すとその変更が失われます。
undo-problem-missing = 取り消せません：{ $file } が見つかりません。
undo-problem-taken = 取り消せません：別のファイルが { $file } という名前になっています。
undo-problem-io = { $file } の変更を取り消せません：{ $reason }

rotate-done-lossless = { $file } を劣化なしで回転しました。
rotate-done-lossless-trimmed = { $file } を劣化なしで回転しました。端を切り詰めて { $width } × { $height } になりました。
rotate-done-tag = 向きのタグを変更して { $file } を回転しました。
rotate-done-sidecar = { $file } の回転を { $sidecar } に保存しました。RAW ファイル自体は変更しません。
rotate-tag-unreadable = 回転に必要なところまで画像データを読み取れませんでした。
rotate-tag-multi-picture = 複数の画像を含んでおり、最初の画像を回転すると他が壊れます。
rotate-tag-trailer = 画像の後ろにデータがあり、回転するとその位置がずれます。
rotate-tag-not-aligned = ピクセルを劣化なしで回転すると { $width } × { $height } に切り詰められます。
rotate-trim-offer = 切り詰めて回転
rotate-problem-unsupported = { $file } は回転を記録できない種類のファイルです。
rotate-problem-too-large = { $file } は大きすぎて回転できません。
rotate-problem-damaged = { $file } の向き情報の部分が壊れています：{ $reason }
rotate-problem-io = { $file } を回転できませんでした：{ $reason }

share-failed = 共有できませんでした：{ $reason }
email-failed = メールアプリを起動できませんでした：{ $reason }
# machine-drafted, needs native review
zynk-opening = Zynk を開いています…
zynk-failed = Zynkに接続できませんでした：{ $reason }
zynk-get-title-send = 「Zynkで送信」には Zynk アプリが必要です
zynk-get-title-share = 「Zynkで共有」には Zynk アプリが必要です
zynk-get-about = Zynk は Mac、Windows、Linux、iPhone、Android 向けの無料アプリです。写真や動画、フォルダ丸ごとでも、サイズを問わず、ほかのデバイスや相手に直接送れます。転送はエンドツーエンドで暗号化され、接続が切れても途中から再開します。Zynk を持っていない相手には、ブラウザでダウンロードできるウェブリンクを作成します。
zynk-get-macos = Mac App Store から入手できます。インストールすると、{ -app-name } からファイルを直接 Zynk に渡せます。
zynk-get-windows = Microsoft Store から入手できます。インストールすると、{ -app-name } からファイルを直接 Zynk に渡せます。
zynk-get-linux = Linux 版 Zynk は AppImage で、インストールは不要です。ダウンロードして ~/Applications に置き、実行可能にしてください（ファイルのプロパティか chmod +x で）。{ -app-name } はそこにある Zynk を見つけます。Zynk の実行中は、ほかの場所にあっても見つけます。
zynk-get = Zynk を入手
zynk-send-old-windows = 「Zynkで送信」には Windows 10 バージョン 2004 以降が必要です。この Windows でも「Zynkで共有」は使えます。
settings-file-export-done = 設定を { $file } に保存しました
settings-file-export-failed = 設定を保存できませんでした：{ $reason }
settings-file-import-unreadable = { $file } を読み取れませんでした：{ $reason }
settings-file-import-done =
    { $count ->
       *[other] { $count } 件の変更を読み込みました
    }
