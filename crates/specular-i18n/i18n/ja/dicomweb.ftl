# Specular — DICOMweb servers, study search and streamed series (DESIGN.md §12.8). 日本語.

palette-dicom-servers = DICOMweb サーバー…

dicomweb-title = DICOMweb サーバー
dicomweb-servers-intro = サーバー上の検査は、要求したときにだけ検索して開きます。それまでは何も送信しません。
dicomweb-servers = サーバー
dicomweb-servers-empty = サーバーはまだありません。DICOMweb アドレスを指定して追加してください。
dicomweb-session-only = パスワードとトークンは Specular を閉じるまでしか保持されません。
dicomweb-add = 追加…
dicomweb-edit = 編集…
dicomweb-remove = 削除
dicomweb-close = 閉じる
dicomweb-browse = 参照
dicomweb-back = 戻る
dicomweb-save = 保存
dicomweb-search = 検索
dicomweb-stop = 停止
dicomweb-removed = { $server } を削除しました
# machine-drafted, needs native review
dicomweb-remove-ask = 「{ $server }」を保存済みサーバーから削除しますか？
dicomweb-remove-detail = ディスク上のものは失われません。このセッションのパスワードまたはトークンは破棄されます。
# 暗号化なし (http://) で接続するサーバーに表示。
dicomweb-not-encrypted = 暗号化なし

# サーバーの追加・編集ページ。
dicomweb-add-title = サーバーを追加
dicomweb-edit-title = サーバー設定
dicomweb-field-name = 名前
dicomweb-hint-name = 任意
dicomweb-field-url = アドレス
dicomweb-hint-url = https://pacs.example.org/dicom-web
dicomweb-field-auth = サインイン
dicomweb-auth-none = サインインなし
dicomweb-auth-basic = ユーザー名とパスワード
dicomweb-auth-bearer = アクセストークン
dicomweb-field-user = ユーザー名
dicomweb-field-password = パスワード
dicomweb-field-token = アクセストークン
dicomweb-hint-secret-kept = 空欄のままにすると入力済みの値を保持します
dicomweb-secret-session = パスワードやトークンは Specular を閉じるまでしか保持されず、ディスクには保存されません。
dicomweb-secret-needed = パスワードまたはトークンを再入力してください。Specular を閉じるまでしか保持されません。
dicomweb-http-warning = このアドレスは暗号化されていません。パスワードやトークンが平文で送信されます。
dicomweb-url-empty = サーバーの DICOMweb アドレスを入力してください。
dicomweb-url-scheme = アドレスは https:// または http:// で始まる必要があります。
dicomweb-url-host = アドレスにサーバー名がありません。
dicomweb-url-userinfo = ユーザー名とパスワードはアドレスではなくサインイン欄に入力してください。
dicomweb-save-failed = サーバー一覧を保存できませんでした: { $reason }

# 検索ページ。
dicomweb-search-title = 検査を検索
dicomweb-field-patient-name = 患者名
dicomweb-field-patient-id = 患者 ID
dicomweb-field-date-from = 開始日
dicomweb-field-date-to = 終了日
dicomweb-hint-date = YYYYMMDD
dicomweb-field-modality = モダリティ
dicomweb-hint-modality = CT, MR…
dicomweb-field-accession = 受付番号
dicomweb-show-patient = 患者データを表示
dicomweb-switch-on = オン
dicomweb-switch-off = オフ
dicomweb-patient-shown = 患者データを表示中
dicomweb-patient-hidden = 患者データを非表示
dicomweb-studies = 検査
dicomweb-search-hint = いずれかの欄に入力して検索してください。
dicomweb-no-studies = 一致する検査はありません。
dicomweb-searching = 検索中…
dicomweb-stopped = 停止しました
dicomweb-load-more = さらに読み込む
dicomweb-study-untitled = 説明のない検査
dicomweb-series-untitled = 説明のないシリーズ
dicomweb-study-counts = { $series } シリーズ、{ $images } 画像
dicomweb-series-images = { $count } 画像
dicomweb-patient-id = ID { $id }
dicomweb-accession = 受付 { $number }
dicomweb-found = { $count } 件の検査が見つかりました
dicomweb-series-found = { $count } シリーズ

# サーバーとの通信エラー。
dicomweb-error-offline = { $server } に接続できません。ネットワークとアドレスを確認してください。
dicomweb-error-auth = サーバーがユーザー名、パスワード、またはトークンを拒否しました。
dicomweb-error-not-found = このアドレスには DICOMweb サービスがありません。
dicomweb-error-other = サーバーが応答できませんでした: { $reason }

# ビューアーにストリーミングされるシリーズ。
dicomweb-loading = { $series } を読み込み中…
dicomweb-loading-progress = { $series } を読み込み中: { $done } / { $total }
dicomweb-loaded = { $series }: { $total } 画像を読み込みました
dicomweb-loaded-failed = { $total } 画像のうち { $failed } 画像を読み込めませんでした。
dicomweb-offline-cached = { $server } に接続できません。以前に保存した画像を表示しています。
dicomweb-cache-failed = ダウンロードフォルダーを開けませんでした: { $reason }
