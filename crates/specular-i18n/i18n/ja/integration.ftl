# Integration: open with, user commands, paste (Stage 2).
palette-open-with = 別のアプリで開く…
palette-paste = 貼り付け
open-with-placeholder = このアプリで開く…
open-with-loading = アプリケーションを検索中…
open-with-default = { $name }（既定）
open-with-choose-other = 別のアプリケーションを選択…
open-with-choose-title = アプリケーションを選択
open-with-failed = { $name } で開けませんでした: { $reason }
command-missing = { $program } が見つかりません
command-failed = { $name } を起動できませんでした: { $reason }
command-exited = { $name } がエラーで終了しました: { $status }
paste-nothing = クリップボードに開けるものがありません
paste-failed = 画像を貼り付けられませんでした: { $reason }
drag-failed = ファイルをドラッグできませんでした: { $reason }
about-storage = 設定: { $settings } · キャッシュ: { $cache }
about-storage-portable = ポータブルモード: 設定とキャッシュは { $dir }

## Scripting and recipes

palette-copy-view-command = 表示をシェルコマンドとしてコピー
palette-copy-view-python = 表示を Python としてコピー
toast-copied-view-command = コマンドをコピーしました
script-copy-no-image = 説明する画像がありません
recipe-confirm-title = レシピ「{ $name }」を初めて実行しますか？実行内容: { $command }
recipe-confirm-run = 実行
recipe-no-file = このレシピには画像ファイルが必要です
recipe-running = { $name } を実行中…
recipe-done = { $name }: { $shapes ->
   *[other] { $shapes } 個の図形
}を追加
recipe-failed = { $name } が失敗しました: { $reason }
recipe-timeout = { $seconds } 秒後に停止
recipe-no-output = { $name } は { $file } を書き出しませんでした
