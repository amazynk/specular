# machine-drafted, needs native review
## マニュアルのパネル、クラッシュレポート、診断パッケージ、アップデートの確認、バージョン情報。日本語。
## 方針: 短く、明快に、余計な言葉なしで。1 文に 1 つの内容。

## パレット

palette-help-manual = ヘルプ
palette-help-manual-help = 画面に表示中のものについてのページでマニュアルを開きます。
palette-save-diagnostics = 診断パッケージを保存…
palette-save-diagnostics-help = クラッシュレポート、ログ、システム情報を zip ファイルに保存します。何も送信されません。
# マニュアルのセクションを開くパレット項目。{ $title } はページまたは「ページ › セクション」。
palette-help-topic = ヘルプ: { $title }
palette-help-topic-help = マニュアルのこのセクションを開きます。
# パレットで使用できないコマンドの下に表示。
palette-disabled-help = 現在は使用できません。Enter キーで理由を表示します。

## ヘルプパネル

help-title = ヘルプ
help-search-placeholder = マニュアルを検索
help-contents = 目次
help-back = 戻る
help-forward = 進む
help-no-results = 「{ $query }」に一致するものはありません。
help-results = { $count ->
   *[other] { $count } 件のセクションが見つかりました
}
help-section-start = はじめに
help-section-modes = 表示とモード
help-section-tasks = 操作方法
help-section-reference = リファレンス
help-keys-unbound = 未設定
# マニュアルに記載された、キーのないコマンド。パレットから実行します。
help-key-unbound = { $name }（パレット）
help-keys-command = コマンド
help-keys-keys = キー
help-commands = すべてのコマンドはコマンドパレット（{ $key }）にあります。名前の一部を入力してください。
help-commands-no-key = すべてのコマンドはコマンドパレットにあります。名前の一部を入力してください。
help-link = { $title } へ移動
help-open = ヘルプ
# スクリーンリーダー用のパネル名。{ $page } は表示中のページ。
help-label = ヘルプ: { $page }
help-disabled-why = 理由

## 次回起動時に表示するクラッシュレポート

crash-title = { -app-name } が予期せず終了しました
crash-body = レポートをこのコンピューターに保存しました。何も送信されていません。
crash-view = レポートを表示
crash-hide = レポートを隠す
crash-copy = コピー
crash-copied = レポートをコピーしました
crash-save-bundle = 診断パッケージを保存…
crash-dismiss = 閉じる

## 診断パッケージ

diag-title = 診断パッケージを保存
diag-intro = 不具合報告に添付する zip ファイルです。このコンピューターに保存されます。何も送信されません。
diag-contents = 内容
diag-include-names = ファイル名を含める
diag-include-names-help = オフ: 名前は file-1.jpg、dir-1 などに置き換えられます。
diag-privacy = ホームフォルダーは ~、ユーザー名は <user> と表示されます。パスワード、サーバーのアドレス、画像は一切含まれません。
diag-where = 問題を報告: { $issues } · { $email }
diag-save = 保存…
diag-cancel = キャンセル
diag-saved = 診断パッケージを保存しました: { $path }
diag-failed = 診断パッケージを保存できませんでした: { $reason }
diag-entry-summary = バージョン、システム、GPU、ドライバー
diag-entry-crash = 最新のクラッシュレポート
diag-entry-crashes = 以前のクラッシュレポート（{ $count }）
diag-entry-log = ログの最後の { $count } 行
diag-entry-settings = 設定（パスワードとアドレスを除く）
diag-entry-gpu = GPU とディスプレイの情報
diag-entry-readme = このパッケージについて
diag-size = { $size }
# パッケージ内の README.txt。
diag-readme =
    { -app-name } 診断パッケージ。

    問題の調査に役立つものが入っています: バージョンとシステムの情報、クラッシュレポート、ログの末尾、設定。

    ホームフォルダーは ~、ユーザー名は <user> と表示されます。含めることを選ばない限り、ファイル名とフォルダー名は file-1、dir-1 などに置き換えられます。パスワードとサーバーのアドレスは削除されます。

    何も送信されていません。このファイルを共有するかどうかはあなたが決めます。

## アップデート（設定）

settings-section-updates = アップデート
settings-updates-check = アップデートを確認
settings-updates-check-help = 1 日に 1 回、新しいバージョンがあるか問い合わせます。送信するのはバージョン番号だけで、識別子も Cookie もありません。オフ: { -app-name } はアップデートのために接続しません。
settings-updates-channel = チャンネル
settings-updates-channel-help = 安定版: リリースのみ。ベータ版: テスト版も含む。
settings-updates-channel-stable = 安定版
settings-updates-channel-beta = ベータ版
settings-updates-check-now = 今すぐ確認
settings-updates-check-now-help = 今すぐ 1 回確認します。
update-available = バージョン { $version } が利用できます
update-notes = リリースノート
update-download = ダウンロード
update-none = { -app-name } は最新です。
update-checking = アップデートを確認しています…
update-failed = アップデートを確認できませんでした: { $reason }
update-last-checked = 最終確認: { $when }

## バージョン情報

about-build = ビルド { $commit } · { $profile } · { $target }
about-build-no-commit = ビルド { $profile } · { $target }
about-help = ヘルプ
about-diagnostics = 診断パッケージを保存…
# machine-drafted, needs native review
about-renderer = レンダラー情報…
about-licences-count = { $count ->
   *[other] サードパーティ製コンポーネント { $count } 件
}
about-licence-group = { $licence }（{ $count }）
about-contact = 不具合: { $issues } · サポート: { $email }
