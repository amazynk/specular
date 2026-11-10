# Figures, key images and teaching files.

## Command palette
# machine-drafted, needs native review
palette-export-figure = 図をエクスポート…
# machine-drafted, needs native review
palette-export-figure-help = スケールバー、カラーバー、ラベル付きの図としてビューを保存します。
# machine-drafted, needs native review
palette-toggle-key-image = キー画像としてマーク
# machine-drafted, needs native review
palette-toggle-key-image-help = この画像をキー画像としてマークします。マーク済みの場合は解除します。
# machine-drafted, needs native review
palette-copy-key-images = キー画像をコピー
# machine-drafted, needs native review
palette-copy-key-images-help = キー画像を 1 枚の画像としてコピーします。
# machine-drafted, needs native review
palette-export-key-images = キー画像をエクスポート…
# machine-drafted, needs native review
palette-export-key-images-help = キー画像をスライドまたは PNG のフォルダーとして保存します。
# machine-drafted, needs native review
palette-export-teaching-file = 教育用ファイルをエクスポート…
# machine-drafted, needs native review
palette-export-teaching-file-help = 患者情報を除いたキー画像を、教育用に保存します。

## The figure dialog
# machine-drafted, needs native review
figure-dialog-title = 図のエクスポート
# machine-drafted, needs native review
figure-format = 形式
# machine-drafted, needs native review
figure-format-png = PNG、16 ビット
# machine-drafted, needs native review
figure-format-tiff = TIFF、16 ビット
# machine-drafted, needs native review
figure-region = 領域
# machine-drafted, needs native review
figure-region-visible = 表示範囲
# machine-drafted, needs native review
figure-region-whole = 画像全体
# machine-drafted, needs native review
figure-size = サイズ
# machine-drafted, needs native review
figure-size-pixels = 画像ピクセル (1:1)
# $width is a width in pixels.
# machine-drafted, needs native review
figure-size-width = 幅 { $width } px
# machine-drafted, needs native review
figure-scale-bar = スケールバー
# machine-drafted, needs native review
figure-scale-bar-none = キャリブレーションなし
# machine-drafted, needs native review
figure-colour-bar = カラーバー
# machine-drafted, needs native review
figure-colour-bar-none = カラーマップもウィンドウもなし
# machine-drafted, needs native review
figure-inset = インセット
# machine-drafted, needs native review
figure-inset-rectangle = 直前の長方形
# machine-drafted, needs native review
figure-inset-none = 先に長方形を描いてください
# machine-drafted, needs native review
figure-channels = チャンネル
# machine-drafted, needs native review
figure-channels-merged = 合成
# machine-drafted, needs native review
figure-channels-split = 分割と合成
# machine-drafted, needs native review
figure-channels-one = 1 チャンネル
# machine-drafted, needs native review
figure-measurements = 計測
# $count is the number of measurements.
# machine-drafted, needs native review
figure-measurements-on = { $count ->
   *[other] オン ({ $count })
}
# machine-drafted, needs native review
figure-measurements-none = なし
# machine-drafted, needs native review
figure-title-row = タイトル
# machine-drafted, needs native review
figure-provenance = 来歴
# machine-drafted, needs native review
figure-provenance-metadata = ファイル内
# machine-drafted, needs native review
figure-provenance-drawn = ファイル内と描画
# machine-drafted, needs native review
figure-file = ファイル
# machine-drafted, needs native review
figure-choose = 選択…
# machine-drafted, needs native review
figure-on = オン
# machine-drafted, needs native review
figure-off = オフ
# $source is where the value comes from (a calibration, a colour map).
# machine-drafted, needs native review
figure-on-from = オン · { $source }
# machine-drafted, needs native review
figure-export = エクスポート
# machine-drafted, needs native review
figure-working = エクスポート中…
# machine-drafted, needs native review
figure-not-full-res = フル解像度のピクセルは読み込まれていません。{ PERCENT(100) } に拡大するか、サイズを選んでください。
# machine-drafted, needs native review
figure-channel-red = 赤
# machine-drafted, needs native review
figure-channel-green = 緑
# machine-drafted, needs native review
figure-channel-blue = 青
# $map is a colour map's name; $lo and $hi the values at its ends; $unit their unit.
# machine-drafted, needs native review
figure-legend-map = { $map }、{ $lo } から { $hi } { $unit }
# $lo and $hi are the window's ends; $unit their unit (HU).
# machine-drafted, needs native review
figure-legend-window = ウィンドウ { $lo } から { $hi } { $unit }
# $preset is a channel-colour preset's id.
# machine-drafted, needs native review
figure-display-channels = チャンネルカラー { $preset }
# $channel is the channel shown (red, green, blue, alpha, luma).
# machine-drafted, needs native review
figure-display-channel = チャンネル { $channel }
# machine-drafted, needs native review
figure-display-hilo = HiLo マーク
# machine-drafted, needs native review
figure-display-as-stored = 保存時のまま
# $value is the calibration (source and spacing).
# machine-drafted, needs native review
figure-caption-calibration = キャリブレーション: { $value }
# $value is the display settings.
# machine-drafted, needs native review
figure-caption-display = 表示: { $value }
# machine-drafted, needs native review
figure-not-image = 図はビューアーで開いている画像から作成します。
# machine-drafted, needs native review
figure-not-ready = 画像を読み込み中です。
# $width and $height are in pixels.
# machine-drafted, needs native review
figure-too-large = 大きすぎます: { $width } × { $height } px。表示範囲またはサイズを選んでください。

## Key images
# machine-drafted, needs native review
figure-keys-title = キー画像のエクスポート
# machine-drafted, needs native review
figure-teaching-title = 教育用ファイルのエクスポート
# machine-drafted, needs native review
figure-keys-images = 画像
# $count is the number of key images.
# machine-drafted, needs native review
figure-keys-count = { $count ->
   *[other] { $count } 枚のキー画像
}
# machine-drafted, needs native review
figure-keys-pptx = スライド (PPTX)
# machine-drafted, needs native review
figure-keys-folder = PNG のフォルダー
# machine-drafted, needs native review
figure-keys-preset = プリセット
# machine-drafted, needs native review
figure-keys-standard = 標準
# machine-drafted, needs native review
figure-keys-teaching = 教育用ファイル
# machine-drafted, needs native review
figure-keys-overlays-on = 描画あり、ノートにも記載
# machine-drafted, needs native review
figure-keys-masks = 焼き込み文字
# machine-drafted, needs native review
figure-keys-masks-auto = 黒塗り
# machine-drafted, needs native review
figure-keys-notes = 症例ノート
# machine-drafted, needs native review
figure-keys-destination = 保存先
# machine-drafted, needs native review
figure-keys-empty = キー画像はまだありません。先に画像をキー画像としてマークしてください。
# machine-drafted, needs native review
figure-keys-teaching-note = 患者情報は除外され、焼き込み文字は黒塗りされます。共有する前に、すべての画像を確認してください。
# machine-drafted, needs native review
figure-keys-none-left = 患者情報なしでエクスポートできる画像がありませんでした。
# machine-drafted, needs native review
figure-keys-copied = キー画像をコピーしました
# $count is how many key images there are now.
# machine-drafted, needs native review
figure-key-marked = { $count ->
   *[other] キー画像としてマークしました ({ $count })
}
# $count is how many key images are left.
# machine-drafted, needs native review
figure-key-unmarked = { $count ->
   *[other] キー画像ではなくなりました (残り { $count })
}
# $n is a frame or page number.
# machine-drafted, needs native review
figure-key-frame = 画像 { $n }
# $n is the key image's number in the set.
# machine-drafted, needs native review
figure-key-n = キー画像 { $n }

## Results
# $name is the file or folder written.
# machine-drafted, needs native review
figure-saved = { $name } を保存しました
# $name is the file or folder written; $count how many images were left out.
# machine-drafted, needs native review
figure-saved-skipped = { $count ->
   *[other] { $name } を保存しました。{ $count } 枚の画像は除外されました。焼き込み文字が見つかりませんでした。
}
# $reason says why.
# machine-drafted, needs native review
figure-failed = エクスポートできませんでした: { $reason }
# $name is a file that is already in the folder.
# machine-drafted, needs native review
figure-folder-taken = { $name } はそのフォルダーにすでにあります。空のフォルダーを選んでください。

## The viewer
# machine-drafted, needs native review
figure-key-badge = キー画像
