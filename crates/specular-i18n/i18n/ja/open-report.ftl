# machine-drafted, needs native review
# Specular — 読み込みレポート：画像を開くのにかかった時間の内訳（情報パネルの「パフォーマンス」
# セクション。DESIGN.md §12.7）。

open-report-title = パフォーマンス

## Headlines
open-report-fast = { $ms } ms で開きました
open-report-display-cache = 表示キャッシュから表示しました
open-report-slow-read = 低速なストレージ：時間の { $percent } がファイルの読み込みにかかりました（{ $speed } MB/s）
# machine-drafted, needs native review
open-report-slow-read-no-speed = 低速なストレージ：時間の { $percent } がファイルの読み込みにかかりました（小さいファイル：主にストレージの応答待ち）
open-report-archive-read = 時間の { $percent } がアーカイブの展開にかかりました
open-report-startup = 時間の { $percent } が { -app-name } の起動にかかりました
open-report-checked = 2 回デコード：ハードウェアデコーダーの最初の結果を CPU の結果と照合しました（時間の { $percent }）
open-report-hardware = ハードウェアデコーダーでデコード（時間の { $percent }）
open-report-decode = 大きな画像：{ $mpx } メガピクセルのデコードに時間の { $percent } がかかりました
open-report-upload = 時間の { $percent } が GPU への転送にかかりました
open-report-waiting = 時間の { $percent } がほかの処理の待ち時間でした
open-report-stage = { $stage }：時間の { $percent }
open-report-memory-short = メモリが不足していたため、スワップが発生した可能性があります

## Why a decode ran on the processor although a hardware decoder can do it
open-report-sw-not-probed = ソフトウェアでデコード：ハードウェアデコーダーを検出中でした
open-report-sw-no-device = ソフトウェアでデコード：このコンピューターでは { $api } を利用できません
open-report-sw-no-codec = ソフトウェアでデコード：この GPU の { $api } には { $codec } デコーダーがありません
open-report-sw-blocked-mismatch = ソフトウェアでデコード：結果が CPU と一致しなかったため、ハードウェアデコーダーを無効にしました
open-report-sw-blocked-error = ソフトウェアでデコード：エラーのため、ハードウェアデコーダーを無効にしました
open-report-sw-blocked-lost = ソフトウェアでデコード：応答しなくなったため、ハードウェアデコーダーを無効にしました
open-report-sw-failed = ソフトウェアでデコード：この画像でハードウェアデコーダーが失敗しました
open-report-sw-busy = ソフトウェアでデコード：ハードウェアデコーダーが使用中でした
open-report-sw-unsupported = ソフトウェアでデコード：ハードウェアデコーダーはこの画像に対応していません
open-report-sw-unavailable = ソフトウェアでデコード：{ -app-name } を再起動するまでハードウェアデコードは無効です
open-report-sw-cpu-only = ソフトウェアでデコード：処理が「CPU のみ」に設定されています
open-report-sw-setting-off = ソフトウェアでデコード：設定でハードウェアデコードが無効になっています
open-report-sw-unchecked = ソフトウェアでデコード：ハードウェアデコーダーはまだ照合されていません（まずバックグラウンドで準備する画像で試されます）
open-report-sw-lower-power = ソフトウェアでデコード：バッテリー駆動時は CPU のほうが消費電力が少なくなります
open-report-sw-faster = ソフトウェアでデコード：このコンピューターではこちらが高速です（約 { $cpu } ms、ハードウェアデコーダーでは { $hw } ms）
open-report-sw-scaled = ソフトウェアでデコード：ハードウェアデコーダーはフルサイズでしかデコードできません

## Hints
open-report-hint-copy-local = ファイルをローカルディスクにコピーすると速く開けます。
open-report-hint-extract = アーカイブをフォルダーに展開すると、ページを速く開けます。
open-report-hint-try-again = ハードウェアデコーダーを再度試すには：設定 → パフォーマンス → 再試行。
open-report-hint-automatic = 設定 → パフォーマンスで「画像を処理する場所」を「自動」にしてください。
open-report-hint-turn-on = 設定 → パフォーマンスで「ハードウェアデコード」を「自動」にしてください。
open-report-hint-free-memory = ほかのプログラムを閉じてメモリを空けてください。
open-report-hint-once-only = このような照合は最初の画像だけです。

## Rows
open-report-total = 鮮明になるまで
open-report-first-pixels = 最初の表示
# machine-drafted, needs native review
open-report-whole = 画像全体を表示
open-report-stage-startup = 起動
open-report-stage-read = ファイルの読み込み
open-report-stage-parse = ヘッダーの読み込み
open-report-stage-colour = カラーマネジメントの準備
open-report-stage-decode = デコード
open-report-stage-upload = GPU への転送
open-report-stage-waiting = ほかの処理の待機
open-report-decoder = デコード
open-report-decoder-checked = { $engine }（CPU で照合）
open-report-decoder-cache = 表示キャッシュ
# $scale is the reduction: 4 for a level a quarter of the full width and height.
open-report-decoder-scaled = { $engine }（1/{ $scale } サイズ）
open-report-decoder-scaled-full = { $engine }（フルサイズでデコード後、1/{ $scale } に縮小）
open-report-decoder-scaled-no-index = { $engine }（1/{ $scale } サイズ、クイックスタート用インデックスなし）
open-report-colour-conversion = 色変換

## Advisor: a pattern across recent opens, what would help and by how much (rows)
# $count and $total are counts of opens, $format a file format ("HEIC"), $speed a formatted
# number (MB/s), $low and $high formatted numbers ("2.5").
open-report-advice-recurring = 繰り返し発生
open-report-advice-change = 改善策
open-report-advice-gain = 推定効果
open-report-advice-software = 直近の低速な { $format } 画像 { $total } 枚のうち { $count } 枚が、この理由でソフトウェアでデコードされました
open-report-advice-slow-storage = 直近の低速な画像 { $total } 枚のうち { $count } 枚が、ファイルの読み込みで遅れました（約 { $speed } MB/s）
# machine-drafted, needs native review
open-report-advice-slow-storage-no-speed = 直近の低速な画像 { $total } 枚のうち { $count } 枚が、ファイルの読み込みで遅れました（小さいファイル：主にストレージの応答待ち）
open-report-advice-archive = 直近の低速な画像 { $total } 枚のうち { $count } 枚が、アーカイブの展開で遅れました
open-report-advice-hardware-decoder = { $codec } ハードウェアデコーダーを搭載した GPU
open-report-advice-gain-range = 約 { $low }～{ $high } 倍高速
open-report-advice-gain-one = 約 { $low } 倍高速

## Performance insights: the indicator in the corner and the panel it opens
# machine-drafted, needs native review
# $total is a formatted duration ("240 ms"), $format a file format ("HEIC").
open-report-insights-title = パフォーマンスのヒント
open-report-insights-indicator = 遅い原因は？
open-report-insights-announce = パフォーマンスのヒント：画像が同じ理由で繰り返し遅く開かれています。「遅い原因は？」を開くと改善策を確認できます。
open-report-insights-hide = 非表示にする
open-report-insights-last = 直近の低速な画像：鮮明になるまで { $total }
open-report-insights-now = 今すぐできること
open-report-insights-hardware = 効果のあるハードウェア
open-report-insights-none = 繰り返し発生している遅延はありません
open-report-insights-none-detail = 複数の画像が同じ理由で遅く開かれると、ヒントが表示されます。これを見つけるため、{ -app-name } は低速だった表示の記録を 1 週間分このコンピューターに保存します。どこにも送信されません。
open-report-insights-software = { $format } 画像が CPU でデコードされています
open-report-insights-storage = ファイルの読み込みで画像が遅れています
open-report-insights-archive = アーカイブの展開で画像が遅れています
open-report-insights-nothing-now = { -app-name } の設定ではこれを変更できません。
open-report-insights-display-cache = 表示キャッシュをオンにしてください：一度開いた画像は、このコンピューター上の高速なコピーから開かれます。
open-report-insights-prefetch = 隣接する画像をより多く準備しておくと、見ている間に次の画像が準備されます。
open-report-insights-hw-ssd = これらのファイル用の高速な内蔵ドライブ（NVMe SSD）
open-report-insights-action-turn-on = オンにする
open-report-insights-action-try-again = 再試行
open-report-insights-action-automatic = 「自動」にする
open-report-insights-dismiss-storage = ストレージについて今後表示しない
open-report-insights-dismiss-decoder = デコードについて今後表示しない
open-report-insights-off = ヒントをオフにする
open-report-insights-on = ヒントをオンにする
open-report-insights-restore = 非表示にしたヒントを再表示する
open-report-insights-power = このコンピューターは省電力中のため（バッテリー駆動または省電力モード）、CPU でのデコードが遅くなっています。電源に接続すると、画像が速く開く場合があります。

## Performance insights in the gallery: the same, about making thumbnails
# machine-drafted, needs native review
# $count and $total are counts of thumbnails, $total in -last-thumbnail a formatted duration
# ("240 ms"), $format a file format ("HEIC"), $speed a formatted number (MB/s).
open-report-insights-announce-thumbnails = パフォーマンスのヒント：サムネイルが同じ理由で繰り返し遅く表示されています。「遅い原因は？」を開くと改善策を確認できます。
open-report-insights-last-thumbnail = 直近の低速なサムネイル：作成に { $total }
open-report-insights-thumbnails-software = { $format } 画像のサムネイルが CPU でデコードされています
open-report-insights-thumbnails-storage = ファイルの読み込みでサムネイルが遅れています
open-report-insights-thumbnails-archive = アーカイブの展開でサムネイルが遅れています
open-report-insights-thumbnail-cache = このコンピューターにサムネイルを保存できないため（サムネイルキャッシュを利用できません）、フォルダを表示するたびにサムネイルが作り直されます。
open-report-advice-thumbnails-software = 直近の低速な { $format } サムネイル { $total } 枚のうち { $count } 枚が、この理由でソフトウェアでデコードされました
open-report-advice-thumbnails-slow-storage = 直近の低速なサムネイル { $total } 枚のうち { $count } 枚が、ファイルの読み込みで遅れました（約 { $speed } MB/s）
# machine-drafted, needs native review
open-report-advice-thumbnails-slow-storage-no-speed = 直近の低速なサムネイル { $total } 枚のうち { $count } 枚が、ファイルの読み込みで遅れました（小さいファイル：主にストレージの応答待ち）
open-report-advice-thumbnails-archive = 直近の低速なサムネイル { $total } 枚のうち { $count } 枚が、アーカイブの展開で遅れました
