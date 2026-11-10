# machine-drafted, needs native review
# Specular — 選別モード (DESIGN.md §6.3).

palette-cull = 選別モードに入る/終了する
cheatsheet-cull = 選別モード
settings-section-cull = 選別モード
settings-cull-auto-enter = RAW フォルダーを選別
settings-cull-auto-enter-help = RAW ファイルが大半を占めるフォルダーを直接選別モードで開きます。
settings-cull-auto-advance = マーク後に次の画像へ
settings-cull-auto-advance-help = 採用・不採用・星評価の後に次へ進みます。ラベルとマーク解除では画像に留まります。
settings-cull-show = RAW+JPEG ペアの表示
settings-cull-show-help = 同じ名前の RAW と JPEG は 1 枚の画像として選別され、マークは両方に適用されます。JPEG のほうが速く開きます。
settings-cull-show-jpeg = JPEG
settings-cull-show-raw = RAW

# machine-drafted, needs native review
settings-cull-bursts = 連写をまとめる
settings-cull-bursts-help = 1 台のカメラで連続して撮影した写真を 1 つのスタックとして表示します。S でスタックを開閉し、B で表示中の画像を残してスタックの残りを不採用にします。
settings-cull-burst-gap = 連写の間隔
settings-cull-burst-gap-help = 1 つの連写内の 2 枚の間の最大の間隔。
settings-cull-watch = 新しいファイルを表示
settings-cull-watch-help = 選別中にフォルダーに現れたファイル (接続したカメラ、取り込み) は、書き込みが完了すると末尾に追加されます。
settings-cull-follow-newest = 最新のファイルを追う
settings-cull-follow-newest-help = 新しいファイルが届くたびにそこへ移動します。選別中は T で切り替えます。
settings-cull-slot-1 = フォルダー 1
settings-cull-slot-1-help = Alt+1 で画像 (RAW または JPEG とサイドカーファイルを含む) をこのフォルダーにコピーし、Alt+Shift+1 で移動します。
settings-cull-slot-1-unset = 未設定
settings-cull-slot-2 = フォルダー 2
settings-cull-slot-2-help = Alt+2 で画像 (RAW または JPEG とサイドカーファイルを含む) をこのフォルダーにコピーし、Alt+Shift+2 で移動します。
settings-cull-slot-2-unset = 未設定
settings-cull-slot-3 = フォルダー 3
settings-cull-slot-3-help = Alt+3 で画像 (RAW または JPEG とサイドカーファイルを含む) をこのフォルダーにコピーし、Alt+Shift+3 で移動します。
settings-cull-slot-3-unset = 未設定
settings-cull-collisions = 名前が使用済みのとき
settings-cull-collisions-help = フォルダー内のファイルが置き換えられることはありません。
settings-cull-collisions-suffix = 番号を付ける
settings-cull-collisions-skip = 画像をスキップ
# machine-drafted, needs native review
# フォルダーもギャラリー画像も開いていない状態で選別モードを選んだとき。
cull-nothing = 選別する写真がありません — まず写真のフォルダーを開いてください
cull-slot-unset = フォルダー { $slot } が未設定です — 設定 → 選別モードで選んでください
cull-slot-missing = フォルダー { $slot } を使用できません: { $reason }
cull-copied = { $folder } にコピーしました
cull-moved = { $folder } に移動しました
cull-slot-renamed = { $folder } に { $name } として保存しました
cull-slot-skipped = { $folder } には既に { $name } があります — スキップしました
cull-slot-same = 画像は既に { $folder } にあります
cull-slot-failed = { $op ->
    [move] { $file } を移動できませんでした
   *[copy] { $file } をコピーできませんでした
}: { $reason }
cull-copy-kept = その後変更されたコピー { $count } 件を残しました
cull-undo-failed = { $file } を元に戻せませんでした: { $reason }
cull-burst-open = 連写 { $index } / { $count }
cull-burst-closed = { $count } 枚の連写
cull-burst-toggle = 連写を開閉
cull-burst-best = 1 枚を残し、{ $count } 枚を不採用
cull-burst-none = 連写の一部ではありません
cull-follow-on = 新しいファイルを追っています
cull-follow-off = 新しいファイルを追っていません
cull-new-files = 新しいファイル { $count } 件
cull-key-copy = フォルダー 1–3 にコピー
cull-key-move = フォルダー 1–3 に移動
cull-key-burst = 連写を開閉
cull-key-best = これを残し、連写の残りを不採用
cull-key-follow = 新しいファイルを追う
cull-key-subframes = サブフレーム表を表示/非表示
cull-key-sort = 次の列で並べ替え
cull-key-sort-reverse = 順序を反転
cull-key-metric = 次の指標をグラフに表示
cull-key-sigma-down = 自動不採用: k を下げる
cull-key-sigma-up = 自動不採用: k を上げる
cull-key-auto-reject = 提案されたフレームを不採用にする
cull-key-move-rejects = 不採用のフレームをサブフォルダーに移動
cull-key-same-stretch = すべてのフレームを同じストレッチで表示

## Strip tooltips. machine-drafted, needs native review
cull-tip-burst-open = 連写を開く
cull-tip-burst-close = 連写を閉じる
cull-tip-exit = 選別モードを終了
cull-tip-label = ラベル: { $label }

## Astronomy subframes. machine-drafted, needs native review
cull-astro-panel = サブフレーム
cull-astro-title = サブフレーム { $count } 枚
cull-astro-pending = { $count } 枚を測定中
cull-astro-col-name = フレーム
cull-astro-col-filter = フィルター
cull-astro-col-exposure = 露出
cull-astro-col-hfd = HFD
cull-astro-col-fwhm = FWHM
cull-astro-col-ecc = 離心率
cull-astro-col-stars = 星数
cull-astro-col-bg = 背景
cull-astro-col-snr = SNR
cull-astro-col-noise = ノイズ
cull-astro-same-stretch = 共通ストレッチ
cull-astro-own-stretch = 個別ストレッチ
cull-astro-suggest-high = { $metric } が { $value } を超過 (中央値 + { $k }σ): { $count } 枚
cull-astro-suggest-low = { $metric } が { $value } 未満 (中央値 − { $k }σ): { $count } 枚
cull-astro-suggest-none = { $k }σ を超えるフレームはありません
cull-astro-suggest-few = 自動不採用には測定済みのフレームが 3 枚必要です
cull-astro-suggested = 不採用の候補
cull-astro-k = k { $k }
cull-astro-reject = { $count } 枚を不採用
cull-astro-move = 不採用を移動…
cull-astro-chart = { $count } 枚のフレームの { $metric }、中央値 { $median }。
cull-astro-sort-by = { $column } で並べ替え
cull-astro-ascending = 昇順
cull-astro-descending = 降順
cull-astro-sorted = { $column } で並べ替えました
cull-astro-reversed = 順序を反転しました
cull-astro-metric = グラフ: { $metric }
cull-astro-k-set = 自動不採用: { $k }σ
cull-astro-none = ここにサブフレームはありません
cull-astro-same-on = すべてのフレームを同じストレッチで表示
cull-astro-same-off = フレームごとに個別ストレッチ
cull-astro-line-hfd = HFD { $value } px
cull-astro-line-fwhm = FWHM { $value } px
cull-astro-line-fwhm-arcsec = FWHM { $value } px ({ $arcsec }″)
cull-astro-line-ecc = 離心率 { $value }
cull-astro-line-stars = 星 { $count } 個
cull-astro-measuring = 星を測定中…
cull-astro-reject-confirm = 提案された { $count } 枚のフレームを不採用にしますか？
cull-astro-reject-detail = { $rule }。元に戻すとマークが戻ります。
cull-astro-reject-ok = 不採用
cull-astro-nothing-to-reject = 不採用にするフレームはありません
cull-astro-rejected = { $count } 枚を不採用にしました
cull-move-rejects-confirm = 不採用の { $count } 枚のフレームをフォルダー「{ $folder }」に移動しますか？
cull-move-rejects-detail = ファイルは削除されず、移動されます。元に戻すと元の場所に戻ります。
cull-move-rejects-ok = 移動
cull-move-rejects-none = 移動する不採用のフレームはありません
cull-moved-rejects = { $count } 枚のフレームを「{ $folder }」に移動しました
