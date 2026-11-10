# Specular — OMERO servers and the Image Data Resource (IDR), browsed read-only. 日本語. machine-drafted, needs native review

palette-omero-servers = OMERO サーバー…

omero-title = OMERO サーバー
omero-servers-intro = サーバー上のプロジェクト、スクリーン、画像は、参照したときにだけ一覧表示されます。それまでは何も送信しません。
omero-servers = サーバー
omero-servers-empty = サーバーはまだありません。OMERO.web アドレスを指定して追加してください。
omero-session-only = パスワードは Specular を閉じるまでしか保持されません。
omero-add = 追加…
omero-edit = 編集…
omero-remove = 削除
omero-close = 閉じる
omero-browse = 参照
omero-back = 戻る
omero-save = 保存
omero-stop = 停止
omero-open = 開く
# $server a server's name.
omero-removed = { $server } を削除しました
# Shown with servers reached without encryption (http://).
omero-not-encrypted = 暗号化なし
# A server browsed without signing in.
omero-public = 公開
# $user the user name a server is signed in with.
omero-signed-in-as = { $user } としてサインイン

# The page that adds or edits a server.
omero-add-title = サーバーを追加
omero-edit-title = サーバー設定
omero-field-name = 名前
omero-hint-name = 任意
omero-field-url = アドレス
omero-hint-url = https://omero.example.org
omero-field-user = ユーザー名
omero-hint-user = 公開データの場合は空欄
omero-field-password = パスワード
omero-hint-password-kept = 入力済みのものを使う場合は空欄のまま
omero-password-session = パスワードは Specular を閉じるまでしか保持されず、ディスクには保存されません。
omero-password-needed = パスワードを入力してください。Specular を閉じるまでしか保持されません。
omero-http-warning = このアドレスは暗号化されていません。パスワードが平文で送信されます。
omero-url-invalid = https:// または http:// で始まる OMERO.web アドレスを入力してください。
# $reason why the list could not be saved.
omero-save-failed = サーバー一覧を保存できませんでした：{ $reason }

# The browse page.
omero-browse-title = 参照
omero-top = プロジェクトとスクリーン
omero-datasets = データセット
omero-plates = プレート
omero-wells = ウェル
omero-fields = フィールド
omero-images = 画像
omero-kind-project = プロジェクト
omero-kind-dataset = データセット
omero-kind-image = 画像
omero-kind-screen = スクリーン
omero-kind-plate = プレート
omero-kind-well = ウェル
# $well a well's row and column, as B7.
omero-well = ウェル { $well }
omero-well-fields = { $count ->
   *[other] { $count } フィールド
}
omero-children = { $count ->
   *[other] { $count } 項目
}
omero-load-more = さらに読み込む
omero-loading = 読み込み中…
omero-empty = 何もありません
omero-stopped = 停止しました
omero-found = { $count ->
   *[other] { $count } 項目
}
# $reason why the image could not be opened.
omero-open-failed = 画像を開けませんでした：{ $reason }
# The channel keys would hide the last channel shown.
omero-channel-last = 少なくとも 1 つのチャンネルは表示したままになります
# $server a server's name.
omero-session-ended = { $server } のセッションが終了しました。続けるにはパスワードを入力してください。

# Errors.
# $server a server's name.
omero-error-offline = { $server } に接続できませんでした。
omero-error-sign-in = このサーバーにはサインインが必要です。
omero-error-expired = セッションが終了しました。
omero-error-login = ユーザー名またはパスワードが拒否されました。
omero-error-forbidden = このアカウントでは表示できません。
omero-error-not-found = サーバー上に見つかりません（削除された可能性があります）。
# $code an HTTP status code.
omero-error-status = サーバーがエラー { $code } を返しました。
# $reason what was wrong with the answer.
omero-error-malformed = サーバーの応答を読み取れませんでした：{ $reason }
