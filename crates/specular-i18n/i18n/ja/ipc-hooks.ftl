# machine-drafted, needs native review
# Remote control and events (Settings → Integration), and hooks: programs settings.toml
# names to run when something happens in the viewer.

settings-integration-ipc = リモート操作とイベント
settings-integration-ipc-help = このコンピューター上であなたのユーザーとして動く他のプログラムは、専用のソケット (specular ipc) を通じて { -app-name } を操作し、その中で起きたことを受け取れます。ネットワークでは何も待ち受けません。

## The hooks panel under "Remote control and events".
hooks-panel-title = フック
hooks-panel-intro = 何かが起きたときに settings.toml が実行するプログラムです。それぞれ初回の実行前に一度確認します。
hooks-panel-none = フックはありません。settings.toml に [[hooks]] として追加します。
hooks-panel-keywords = フック hooks イベント 自動化 スクリプト プログラム
# $events: the event names, joined ("marked, exported").
hooks-panel-on = { $events }とき
hooks-panel-untrusted = 初回の実行前に確認
hooks-panel-allowed = 許可済み
hooks-panel-remove = 削除…
hooks-panel-unused = 未使用

## Asking before a hook's first run (and again when its program or arguments change).
# $name: the hook's name; $program: the program; $events: what sets it off (hook-event-*,
# joined with hook-or).
hook-consent-title = { $events }ときに「{ $name }」({ $program }) を実行しますか?
hook-consent-detail = 写真の名前を付けてこのプログラムを起動します: { $args }
hook-consent-trust = 信頼できるプログラムだけを許可してください。フックが変わるとまた確認します。
hook-consent-allow = 許可
hook-consent-not-now = 今はしない
hook-or = { "" }、または{ "" }
hook-event-opened = 写真を開いた
hook-event-marked = 写真に印を付けた
hook-event-keywords = キーワードが変わった
hook-event-exported = 写真をエクスポートした
hook-event-imported = 写真を読み込んだ
hook-event-captured = カメラが写真を送った
hook-event-develop-saved = 現像の編集を保存した
hook-event-any = 何かが起きた

## Removing a hook (Settings › Integration › Hooks › Remove…).
hook-remove-title = フック「{ $name }」を削除しますか?
hook-remove-detail = settings.toml から取り除きます。プログラムとファイルはディスクに残ります。
hook-remove-ok = 削除
hook-removed = フック「{ $name }」を削除しました。
hook-settings-unwritable = settings.toml を変更できませんでした: { $reason }

## A hook that did not work (a quiet note; the log has the details).
hook-failed = フック「{ $name }」が動作しませんでした: { $reason }
hook-failed-exit = コード { $code } で終了しました
hook-failed-timeout = { $seconds } 秒後に停止しました
hook-failed-start = 起動できませんでした ({ $reason })
hook-dropped = フック「{ $name }」は処理中です: イベントを { $count } 件スキップしました。

## An export started by another program (specular ipc export): nobody was asked.
export-recipes-remote-exists = 置き換えるファイルが { $count } 件あります: 何もエクスポートしていません
export-recipes-remote-skipped = { $file } があります
