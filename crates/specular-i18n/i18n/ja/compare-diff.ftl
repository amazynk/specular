# Compare: split, signed difference, flicker, alignment, metrics and the pixel probe
# (DESIGN.md §6.3).

## Actions (palette, cheat sheet, remote control)
palette-compare-with = 比較する画像を選ぶ…
palette-compare-with-help = この画像を選んだ別の画像と比較します。
palette-compare-side = 左右に並べて比較
palette-compare-side-help = 比較中の画像を並べて表示します。
palette-compare-split = 分割して比較
palette-compare-split-help = 2 枚目の画像を 1 枚目に重ね、ドラッグできる境界線で切り分けて表示します。
palette-compare-split-direction = 境界線の向きを変える
palette-compare-split-direction-help = 境界線を縦から横に、または元に戻します。
palette-compare-overlay = オーバーレイで比較
palette-compare-overlay-help = 2 枚目の画像を 1 枚目にブレンドして重ねます。
palette-compare-difference = 差分で比較
palette-compare-difference-help = 画像が異なる箇所を表示します。もう一度押すと次の種類。
palette-compare-flicker = 比較中の画像をフリッカー表示
palette-compare-flicker-help = もう一方の画像をすぐに表示します。
palette-compare-auto-flicker = 自動フリッカー
palette-compare-auto-flicker-help = 2 枚の画像を自動で切り替えます。または停止します。
palette-compare-align = 比較中の画像を位置合わせ
palette-compare-align-help = サイズの異なる画像を揃えます: 左上、中央、拡大縮小、または自動検出。
palette-compare-metric-space = 測定空間を切り替え
palette-compare-metric-space-help = リニア光またはエンコード済み sRGB で測定します。
palette-compare-whole-image = 画像全体を測定
palette-compare-whole-image-help = 表示範囲ではなく画像全体を測定します。または元に戻します。
palette-compare-probe = 比較ピクセルプローブ
palette-compare-probe-help = ポインター位置の両方の画像の値を表示します。
palette-compare-link = 比較中の画像を連動または解除
palette-compare-link-help = 2 枚の画像をそれぞれ個別に、または再び一緒にズーム・パンします。
palette-compare-close = 比較を閉じる
palette-compare-close-help = ギャラリーに戻ります。
disabled-compare-closed = 先に比較を開いてください
disabled-compare-not-two = 2 枚の画像で使えます

## Cheat sheet
cheatsheet-compare-split = 分割または左右に並べる
cheatsheet-compare-split-direction = 境界線を横または縦に
cheatsheet-compare-auto-flicker = 自動フリッカー
cheatsheet-compare-align = 次の位置合わせ
cheatsheet-compare-space = 測定空間
cheatsheet-compare-whole = 画像全体または表示範囲
cheatsheet-compare-probe = ピクセルプローブ
cheatsheet-compare-link = 2 枚の画像を連動または解除

## Settings
settings-gallery-compare-metric-space = 比較の測定空間
settings-gallery-compare-metric-space-help = 比較で MAE、MSE、PSNR、SSIM を測定する空間です。リニア光は画面の表示どおりです。sRGB は 8-bit ファイルで他のツールと一致します。
settings-gallery-compare-metric-space-linear = リニア光
settings-gallery-compare-metric-space-srgb = エンコード済み sRGB
settings-gallery-compare-flicker-limit = 自動フリッカーの最高速度
settings-gallery-compare-flicker-limit-help = 1 秒あたりの画像切り替えの上限です。3 を超える点滅は発作を引き起こすことがあります。

## Toolbar and captions
compare-split = 分割
compare-split-down = 境界線を縦に
compare-split-across = 境界線を横に
compare-link = 連動
compare-auto-flicker = 自動
compare-flicker-rate = 毎秒 { $rate } 回切り替え
compare-gain = ゲイン ×{ $gain }
compare-probe = プローブ
compare-kind-signed = 符号付き誤差
compare-kind-on-image = 画像上の誤差
compare-align-top-left = 左上
compare-align-centre = 中央
compare-align-scaled = B を A に合わせて拡大縮小
compare-align-auto = 自動位置合わせ
compare-space-linear = リニア光
compare-space-srgb = sRGB
compare-space-help-linear = トーンマッピング後の表示どおり、リニア光で測定しています。ここでの PSNR は通常の 8-bit sRGB の PSNR ではありません。
compare-space-help-srgb = 他のツールが 8-bit ファイルで行うように、エンコード済み sRGB の値で測定しています。SDR の白を超える HDR は白として扱います。
compare-space-help-common = ΔE2000 は常に CIELAB (D65) で、SDR の白を L* 100 とし、HDR はクリップします。{ $size } ピクセルを超える画像は { $size } ピクセルのコピーで測定します。
compare-region-view = 表示範囲
compare-region-whole = 画像全体
compare-ssim = SSIM { $ssim }
compare-ssim-small = SSIM には 11 ピクセル必要です
compare-delta-e = ΔE { $mean } ({ PERCENT(95) }: { $p95 }、最大 { $max })
compare-delta-e-hdr = ΔE 用に HDR をクリップ
compare-aligned-centre = 中央揃え、{ $width } × { $height } を比較
compare-aligned-scaled = B を { $scale }× に拡大縮小、{ $width } × { $height } を比較
compare-aligned-auto = { $angle }° { $scale }× で位置合わせ、{ $width } × { $height } を比較
compare-aligning = 位置合わせ中…
compare-align-failed = これらの画像を位置合わせできませんでした。左上に戻します。
compare-split-label = { $b } を { $a } に重ねて、境界線 { PERCENT($percent) }
compare-showing-which = { $which } を表示中: { $name }
compare-auto-flicker-on = フリッカー中、毎秒 { $rate } 回切り替え
compare-auto-flicker-off = フリッカーを停止
compare-linked = 画像を連動
compare-unlinked = 画像の連動を解除: それぞれ個別にパン
compare-with-title = 比較する画像
compare-probe-delta-e = ΔE2000
compare-probe-a11y = A: { $a }。B: { $b }。ΔE2000 { $delta }。
compare-a11y-divider = 分割の境界線
compare-a11y-space = 測定空間
compare-a11y-align = 位置合わせ
