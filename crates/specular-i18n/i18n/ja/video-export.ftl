# Specular — Export slideshow as video (DESIGN.md §6.5). Japanese. Machine-drafted, needs native review.

palette-export-slideshow = スライドショーをビデオとしてエクスポート…

video-export-title = スライドショーをビデオとしてエクスポート
video-export-resolution = 解像度
# $width and $height in pixels.
video-export-resolution-window = ウィンドウ（{ $width } × { $height }）
video-export-fps = フレームレート
# $fps frames per second, a number.
video-export-fps-value = { $fps } fps
video-export-quality = 品質
video-export-quality-high = 高
video-export-quality-standard = 標準
video-export-quality-compact = コンパクト
video-export-images = { $count ->
   *[other] { $count } 枚の画像（スライドショーの順序、切り替え効果、キャプションを使用）
}
# $length a duration ("2:40"), $size a file size ("38 MB").
video-export-summary = 長さ約 { $length }、サイズ約 { $size }
video-export-export = エクスポート…
video-export-replace = 置き換える
# $file a file name.
video-export-replace-question = { $file } はすでに存在します。置き換えますか？
# $percent a formatted percentage ("42%").
video-export-progress = エクスポート中… { $percent }
video-export-cancelling = キャンセル中…
# $file a file name, $size a file size ("38 MB").
video-export-saved = { $file } を保存しました（{ $size }）
video-export-skipped = { $count ->
   *[other] { $count } 枚の画像を読み込めなかったため、除外しました。
}
video-export-failed = ビデオをエクスポートできませんでした: { $reason }
video-export-cancelled = エクスポートをキャンセルしました
video-export-no-images = どの画像も読み込めませんでした。
video-export-show-in-folder = フォルダに表示

## Toolbar item and compatibility (machine-drafted, needs native review)
settings-viewing-toolbar-items-export-slideshow = スライドショーを動画として書き出す
video-export-compatibility = 互換性
video-export-compatibility-current = 現行のプレーヤー
video-export-compatibility-all = すべてのプレーヤー
