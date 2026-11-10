# machine-drafted, needs native review
# Specular — Develop (non-destructive photo editing). 日本語.

# Shown once when an edit opens and some of its modules were made with a version that no
# longer renders: they were moved to the nearest version that does. $count: modules moved.
# machine-drafted, needs native review
toast-develop-edit-migrated = { $count ->
   *[other] この編集の { $count } 個のモジュールを新しいバージョンに更新しました。見た目が少し変わる場合があります。
}
# Shown when an edit holds modules this version of Specular cannot render (made with a newer
# Specular). They are kept and saved unchanged, but skipped. $count: modules skipped.
# machine-drafted, needs native review
toast-develop-edit-newer = { $count ->
   *[other] この編集の { $count } 個のモジュールには新しいバージョンの Specular が必要です。保持されますが、表示されません。
}

## Develop's controls (sliders, curve, colour wheels). Numbers come formatted in the UI
## language's digits.

# machine-drafted, needs native review
# A slider's value in exposure values. $value: "+0.70".
develop-value-ev = { $value } EV
# machine-drafted, needs native review
# machine-drafted, needs native review
# An angle. $value: "2.5".
develop-value-degrees = { $value }°
# machine-drafted, needs native review
# A colour temperature. $value: "5,000".
develop-value-kelvin = { $value } K
# machine-drafted, needs native review
# The curve editor's selected point. $index, $count: point numbers; $input, $output: "25 %".
develop-curve-point = ポイント { $index }/{ $count }：入力 { $input }、出力 { $output }
# machine-drafted, needs native review
# The curve editor's keys, read by screen readers.
develop-curve-keys = 矢印キーでポイントを移動、Page Up と Page Down で別のポイントを選択、Insert でポイントを追加、Delete で削除します。
# machine-drafted, needs native review
# A colour wheel with no colour.
develop-wheel-neutral = ニュートラル
# machine-drafted, needs native review
# A colour wheel's colour. $hue: degrees; $strength: "20 %".
develop-wheel-value = 色相 { $hue }°、強度 { $strength }
# machine-drafted, needs native review
# A colour wheel's keys, read by screen readers.
develop-wheel-keys = 矢印キーで色を移動、Delete でリセットします。

## Develop's panel.

# machine-drafted, needs native review
develop-title = 現像
# machine-drafted, needs native review
develop-undo = 取り消す
# machine-drafted, needs native review
develop-redo = やり直す
# machine-drafted, needs native review
develop-before-after = 補正前と補正後
# machine-drafted, needs native review
develop-close = 現像を終了
# machine-drafted, needs native review
develop-history = ヒストリー
# machine-drafted, needs native review
# A card's on/off switch. $module: the module's name.
develop-module-on = { $module } オン
develop-card-menu = { $module } のオプション
develop-card-changed = 変更あり
# machine-drafted, needs native review
# The speed-edit readout over the image. $control: "Exposure"; $value: "+0.70 EV".
develop-speed-hud = { $control }：{ $value }
# machine-drafted, needs native review
develop-filmstrip = フィルムストリップ

## Develop's module cards and controls.

# machine-drafted, needs native review
develop-module-exposure = 露光量
develop-module-light = ライト
develop-look-profile = プロファイル
develop-look-standard = 標準
develop-look-film = フィルム
develop-look-natural = ナチュラル
develop-module-grading = カラーグレーディング
develop-add-tool = ツールを追加
# machine-drafted, needs native review
develop-module-white-balance = ホワイトバランス
# machine-drafted, needs native review
develop-module-tone = トーン
# machine-drafted, needs native review
develop-module-contrast = コントラストと階調範囲
# machine-drafted, needs native review
develop-module-colour = カラー
# machine-drafted, needs native review
develop-module-curve = カーブ
# machine-drafted, needs native review
develop-module-crop = 切り抜きと角度補正
# machine-drafted, needs native review
# A module this version has no card for.
develop-module-other = その他のモジュール
# machine-drafted, needs native review
develop-exposure = 露光量
# machine-drafted, needs native review
develop-black-level = ブラックレベル
# machine-drafted, needs native review
develop-temperature = 色温度
# machine-drafted, needs native review
develop-tint = 色かぶり補正
# machine-drafted, needs native review
develop-highlights = ハイライト
# machine-drafted, needs native review
develop-shadows = シャドウ
# machine-drafted, needs native review
develop-contrast = コントラスト
# machine-drafted, needs native review
develop-whites = 白レベル
# machine-drafted, needs native review
develop-blacks = 黒レベル
# machine-drafted, needs native review
# The view transform: how scene brightness becomes screen brightness.
# machine-drafted, needs native review
# machine-drafted, needs native review
# machine-drafted, needs native review
# machine-drafted, needs native review
# HDR card: how far above SDR white the HDR picture may go, and which picture to show.
develop-hdr = HDR
# machine-drafted, needs native review
develop-hdr-limit = HDR上限
# machine-drafted, needs native review
develop-hdr-preview = 表示
# machine-drafted, needs native review
develop-hdr-preview-hdr = HDR
# machine-drafted, needs native review
develop-hdr-preview-sdr = SDR
# machine-drafted, needs native review
develop-vibrance = 自然な彩度
# machine-drafted, needs native review
develop-saturation = 彩度
# machine-drafted, needs native review
develop-colour-wheels = カラーホイール
# machine-drafted, needs native review
develop-curve = トーンカーブ
# machine-drafted, needs native review
develop-module-local-contrast = ローカルコントラスト
# machine-drafted, needs native review
# A switch: exposure set from the photo's own brightness.
develop-auto-exposure = 自動露光
# machine-drafted, needs native review
# How far the tone equalizer's highlights and shadows spread across an area.
develop-smoothing = スムージング
# machine-drafted, needs native review
# Local contrast: fine detail, and larger-scale contrast.
develop-texture = テクスチャ
# machine-drafted, needs native review
develop-clarity = 明瞭度
# machine-drafted, needs native review
# Levels on the curve card: the input that becomes black, the middle and white.
develop-levels-black = 黒点
# machine-drafted, needs native review
develop-levels-mid = 中間点
# machine-drafted, needs native review
develop-levels-white = 白点
# machine-drafted, needs native review
develop-module-detail = ディテール
# machine-drafted, needs native review
develop-module-denoise = ノイズ軽減
# machine-drafted, needs native review
develop-module-sharpen = シャープ
# machine-drafted, needs native review
develop-noise-luminance = 輝度ノイズ
# machine-drafted, needs native review
develop-noise-detail = ノイズのディテール
# machine-drafted, needs native review
develop-noise-colour = カラーノイズ
# machine-drafted, needs native review
develop-sharpen-amount = シャープの適用量
# machine-drafted, needs native review
develop-sharpen-radius = シャープの半径
# machine-drafted, needs native review
develop-sharpen-detail = シャープのディテール
# machine-drafted, needs native review
develop-sharpen-masking = マスク
# machine-drafted, needs native review
develop-capture-amount = キャプチャーシャープ
# machine-drafted, needs native review
develop-capture-radius = キャプチャー半径
# machine-drafted, needs native review
develop-straighten = 角度補正
# machine-drafted, needs native review
# History rows of the crop tool.
develop-crop = 切り抜き
# machine-drafted, needs native review
develop-crop-turn = 回転
# machine-drafted, needs native review
develop-crop-flip = 反転
# machine-drafted, needs native review
# The lens card: the camera's own corrections (its switch) and manual ones.
develop-module-lens = レンズ補正
develop-module-optics = 光学
# machine-drafted, needs native review
develop-lens-distortion = ゆがみ
# machine-drafted, needs native review
develop-lens-vignette = 周辺光量
# machine-drafted, needs native review
# Colour fringes at the edges of the frame (lateral chromatic aberration), per colour.
develop-lens-tca-red = 赤のフリンジ
# machine-drafted, needs native review
develop-lens-tca-blue = 青のフリンジ
# machine-drafted, needs native review
# The Lensfun lens profile for raws without corrections of their own: a picker and a note.
develop-lensfun-profile = レンズプロファイル
# machine-drafted, needs native review
# The picker's first option: the profile matched to the file's lens name.
develop-lensfun-auto = 自動
# machine-drafted, needs native review
develop-lensfun-off = なし
# machine-drafted, needs native review
# $lens: "Canon EF 24-70mm f/2.8L II USM". Lensfun is a name; keep it.
develop-lensfun-used = Lensfun: { $lens }
# machine-drafted, needs native review
develop-lensfun-none-chosen = レンズプロファイルなし。
# machine-drafted, needs native review
# $lens: the lens name the file gives.
develop-lensfun-no-match = { $lens } の Lensfun プロファイルはありません。
# machine-drafted, needs native review
develop-lensfun-no-profile = このレンズの Lensfun プロファイルはありません。
# machine-drafted, needs native review
develop-lensfun-no-name = ファイルにレンズ名がありません。上で選んでください。
# machine-drafted, needs native review
# $count: how many lenses match equally well (2 or more).
develop-lensfun-ambiguous = { $count } 本のレンズが一致します。上で 1 本選んでください。

## The crop tool over the photo (key R). Said to screen readers, or shown over the photo.

# machine-drafted, needs native review
develop-crop-open = 切り抜き。端をドラッグするか、水平にしたい線に沿って Ctrl＋ドラッグします。Enter で確定、Esc で戻ります。
# machine-drafted, needs native review
develop-crop-kept = 切り抜きを確定しました
# machine-drafted, needs native review
develop-crop-cancelled = 切り抜きを元に戻しました
# machine-drafted, needs native review
develop-crop-reset = 切り抜きと角度補正をリセットしました
# machine-drafted, needs native review
# $angle: "-1.5".
develop-crop-straightened = { $angle }° に角度補正しました
# machine-drafted, needs native review
# $aspect: "3:2", or one of the two names below.
develop-crop-aspect = 縦横比 { $aspect }
# machine-drafted, needs native review
develop-crop-aspect-free = 自由
# machine-drafted, needs native review
develop-crop-aspect-original = 元の比率
# machine-drafted, needs native review
develop-crop-turned-right = 右に回転しました
# machine-drafted, needs native review
develop-crop-turned-left = 左に回転しました
# machine-drafted, needs native review
develop-crop-flipped-horizontal = 左右に反転しました
# machine-drafted, needs native review
develop-crop-flipped-vertical = 上下に反転しました
# machine-drafted, needs native review
# The label over the photo. $aspect: as above; $guide: one of the guide names below.
develop-crop-label = 縦横比 { $aspect } · ガイド：{ $guide }
# machine-drafted, needs native review
develop-crop-keys = A 縦横比 · X 縦横入れ替え · O ガイド · [ ] 回転 · H V 反転 · Enter 確定 · Esc 戻る
# machine-drafted, needs native review
develop-crop-guide-thirds = 三分割
# machine-drafted, needs native review
develop-crop-guide-golden = 黄金比
# machine-drafted, needs native review
develop-crop-guide-diagonal = 対角線
# machine-drafted, needs native review
develop-crop-guide-grid = グリッド

## Perspective: keystone and upright (its own card; guides are drawn in the crop tool).

# machine-drafted, needs native review
develop-module-perspective = 遠近補正
# machine-drafted, needs native review
# Upright: how the camera's tilt is found.
develop-persp-upright = アップライト
# machine-drafted, needs native review
develop-persp-mode-off = オフ
# machine-drafted, needs native review
develop-persp-mode-auto = 自動
# machine-drafted, needs native review
develop-persp-mode-level = 水平
# machine-drafted, needs native review
develop-persp-mode-vertical = 垂直
# machine-drafted, needs native review
develop-persp-mode-full = フル
# machine-drafted, needs native review
develop-persp-mode-guided = ガイド付き
# machine-drafted, needs native review
develop-persp-vertical = 垂直方向
# machine-drafted, needs native review
develop-persp-horizontal = 水平方向
# machine-drafted, needs native review
develop-persp-rotate = 回転
# machine-drafted, needs native review
develop-persp-aspect = 縦横比
# machine-drafted, needs native review
develop-persp-scale = 拡大・縮小
# machine-drafted, needs native review
develop-persp-offset-x = 左右
# machine-drafted, needs native review
develop-persp-offset-y = 上下
# machine-drafted, needs native review
# Zoom so no empty edges show.
develop-persp-auto-scale = 余白を出さない
# machine-drafted, needs native review
develop-persp-guides = ガイド
# machine-drafted, needs native review
develop-persp-focal = 焦点距離
# machine-drafted, needs native review
develop-persp-draw-guides = ガイドを描く
# machine-drafted, needs native review
# $n: guides drawn; $max: 4.
develop-persp-draw-guides-n = ガイドを描く ({ $n } / { $max })
# machine-drafted, needs native review
develop-persp-reset = 遠近補正をリセット
# machine-drafted, needs native review
# $mode: one of the mode names above.
develop-persp-mode-set = アップライト：{ $mode }
# machine-drafted, needs native review
develop-persp-analysing = 線を検出しています…
# machine-drafted, needs native review
# $n: the guides drawn.
develop-persp-reset-confirm = 遠近補正をリセットしますか？描いた { $n } 本のガイドは削除されます。取り消すと元に戻ります。
# machine-drafted, needs native review
develop-persp-reset-ok = リセット
# machine-drafted, needs native review
develop-persp-reset-done = 遠近補正をリセットしました
# machine-drafted, needs native review
# $n: the guide's number.
develop-persp-guide-remove-confirm = ガイド { $n } を削除しますか？取り消すと元に戻ります。
# machine-drafted, needs native review
develop-persp-guide-remove-ok = 削除
# machine-drafted, needs native review
develop-persp-guide-removed = ガイドを削除しました
# machine-drafted, needs native review
# $vertical, $horizontal: the slider values.
develop-persp-keystone-now = 垂直方向 { $vertical }、水平方向 { $horizontal }
# machine-drafted, needs native review
# $error: what went wrong.
develop-persp-failed = アップライトに失敗しました：{ $error }
# machine-drafted, needs native review
# $mode: a mode name; the angles in degrees ("2.5").
develop-persp-solved = { $mode }：傾き { $vertical }°、振り { $horizontal }°、回転 { $rotate }°
# machine-drafted, needs native review
# $mode: a mode name.
develop-perspective-few-lines = { $mode } には区別できる直線が 2 本以上必要です。別のモードを試すか、ガイドを描いてください。
# machine-drafted, needs native review
# $n: 4.
develop-persp-guides-full = ガイドは最大 { $n } 本です。端をドラッグすると移動できます。
# machine-drafted, needs native review
develop-persp-guides-on = ガイド。垂直または水平にしたい線に沿ってドラッグします。
# machine-drafted, needs native review
develop-persp-guides-off = 切り抜き
# machine-drafted, needs native review
develop-persp-guide-moved = ガイドを移動しました
# machine-drafted, needs native review
# $n: how many guides now.
develop-persp-guide-added = ガイド { $n } を追加しました
# machine-drafted, needs native review
develop-persp-guides-keys = ドラッグで追加 · 端をドラッグで移動 · Delete で削除 · Shift+R で切り抜き
# machine-drafted, needs native review
# $n: guides drawn; $max: 4.
develop-persp-guides-label = ガイド：{ $n } / { $max }
# machine-drafted, needs native review
# The three colour wheels.
develop-wheel-shadows = シャドウ
# machine-drafted, needs native review
develop-wheel-midtones = 中間調
# machine-drafted, needs native review
develop-wheel-highlights = ハイライト
# machine-drafted, needs native review
# The luminance slider under each colour wheel (read by screen readers).
develop-luminance-shadows = シャドウの輝度
# machine-drafted, needs native review
develop-luminance-midtones = 中間調の輝度
# machine-drafted, needs native review
develop-luminance-highlights = ハイライトの輝度
# machine-drafted, needs native review
# The fourth colour wheel: the whole picture.
develop-wheel-global = グローバル
# machine-drafted, needs native review
develop-luminance-global = グローバルの輝度
# machine-drafted, needs native review
# Colour card: colourfulness changed evenly, and the lightness of colourful areas.
develop-chroma = 色の強さ
# machine-drafted, needs native review
develop-brilliance = 色の明るさ

## Colour mixer: hue, saturation and brightness per colour.

# machine-drafted, needs native review
develop-module-colour-mixer = カラーミキサー
# machine-drafted, needs native review
develop-colour-red = レッド
# machine-drafted, needs native review
develop-colour-orange = オレンジ
# machine-drafted, needs native review
develop-colour-yellow = イエロー
# machine-drafted, needs native review
develop-colour-green = グリーン
# machine-drafted, needs native review
develop-colour-aqua = アクア
# machine-drafted, needs native review
develop-colour-blue = ブルー
# machine-drafted, needs native review
develop-colour-purple = パープル
# machine-drafted, needs native review
develop-colour-magenta = マゼンタ
# machine-drafted, needs native review
# $colour: one of the colour names above.
develop-mixer-hue = { $colour }の色相
# machine-drafted, needs native review
develop-mixer-saturation = { $colour }の彩度
# machine-drafted, needs native review
develop-mixer-brightness = { $colour }の輝度

## Point colour: pick a colour in the photo, then shift it and its neighbours.

# machine-drafted, needs native review
develop-module-point-colour = ポイントカラー
# machine-drafted, needs native review
develop-point-pick = 色を選択
# machine-drafted, needs native review
# On the pick button once a colour is picked.
develop-point-pick-again = 別の色を選択
# machine-drafted, needs native review
# Said and shown while picking.
develop-point-picking = 変更する色をクリックします。Esc でキャンセルします。
# machine-drafted, needs native review
develop-point-picked = 色を選択しました
# machine-drafted, needs native review
develop-point-cancelled = 選択をキャンセルしました
# machine-drafted, needs native review
# How far from the picked colour changes reach: overall, then by hue, saturation and lightness.
develop-point-range = 範囲
# machine-drafted, needs native review
develop-point-hue-range = 色相範囲
# machine-drafted, needs native review
develop-point-saturation-range = 彩度の範囲
# machine-drafted, needs native review
develop-point-lightness-range = 明度の範囲
# machine-drafted, needs native review
develop-point-hue = 色相のシフト
# machine-drafted, needs native review
develop-point-saturation = 彩度のシフト
# machine-drafted, needs native review
develop-point-lightness = 明度のシフト

## Black and white.

# machine-drafted, needs native review
develop-module-black-white = 白黒
# machine-drafted, needs native review
# $colour: a colour name; how light that colour turns in grey.
develop-bw-mix = { $colour }の輝度

## LUT: a colour look-up table file.

# machine-drafted, needs native review
develop-module-lut = LUT
# machine-drafted, needs native review
develop-lut-choose = LUT ファイルを選択…
# machine-drafted, needs native review
# The file chosen. $name: its file name.
develop-lut-file = ファイル：{ $name }
# machine-drafted, needs native review
# The file can't be used. $name: its file name.
develop-lut-unreadable = { $name } を読み込めません
# machine-drafted, needs native review
# The file dialog.
develop-lut-dialog = LUT ファイルを選択
# machine-drafted, needs native review
develop-lut-kind = LUT ファイル
# machine-drafted, needs native review
# The encoding the LUT was made for.
develop-lut-space = 対象の色空間
# machine-drafted, needs native review
develop-lut-space-linear = リニア (Rec.2020)
# machine-drafted, needs native review
develop-lut-space-acescg = ACES リニア (ACEScg)
# machine-drafted, needs native review
develop-lut-space-acescct = ACES ログ (ACEScct)
# machine-drafted, needs native review
develop-lut-space-srgb = 写真 (sRGB)
# machine-drafted, needs native review
develop-lut-space-rec709 = ビデオ (Rec.709)
# machine-drafted, needs native review
develop-lut-interpolation = 方式
# machine-drafted, needs native review
develop-lut-tetrahedral = 高精度
# machine-drafted, needs native review
develop-lut-trilinear = シンプル
# machine-drafted, needs native review
develop-lut-strength = 強度

## Develop's history and status.

# machine-drafted, needs native review
develop-history-original = 元の画像
# machine-drafted, needs native review
# A module switched on or off. $module: its name.
develop-history-on = { $module } オン
# machine-drafted, needs native review
develop-history-off = { $module } オフ
# machine-drafted, needs native review
# Said to screen readers after a click on a history row. $step: the row's name.
develop-at-original = 元の画像に戻りました
# machine-drafted, needs native review
develop-at-step = 戻りました：{ $step }
# machine-drafted, needs native review
develop-undone = 取り消しました：{ $step }
# machine-drafted, needs native review
develop-redone = やり直しました：{ $step }
# machine-drafted, needs native review
develop-entered = 現像。編集でファイルが変更されることはありません。
# machine-drafted, needs native review
develop-left = 現像を終了しました
# machine-drafted, needs native review
develop-split-on = 補正前と補正後を並べて表示
# machine-drafted, needs native review
develop-split-off = 編集のみ
# machine-drafted, needs native review
develop-showing-before = 元の画像を表示中
# machine-drafted, needs native review
develop-showing-after = 編集を表示中
# machine-drafted, needs native review
# Labels over the image.
develop-before = 補正前
# machine-drafted, needs native review
develop-after = 補正後
# machine-drafted, needs native review
develop-working = 現像中…
# machine-drafted, needs native review
develop-preview-failed = このファイルの編集を表示できません

## Develop's commands (palette, cheat sheet, Settings → Keys) and why they are unavailable.

# machine-drafted, needs native review
keys-mode-develop = 現像
# machine-drafted, needs native review
# The cheat sheet's heading over the speed-edit keys.
cheatsheet-develop-speed = スピード編集：押し続けてスクロール
# machine-drafted, needs native review
palette-develop = 現像
# machine-drafted, needs native review
palette-develop-help = ファイルを変更せずにこの写真を編集します。もう一度実行すると終了します。
# machine-drafted, needs native review
palette-develop-before-after = 補正前と補正後
# machine-drafted, needs native review
palette-develop-before-after-help = 元の画像を表示します。もう一度実行すると編集に戻ります。
# machine-drafted, needs native review
palette-develop-split = 補正前と補正後を分割
# machine-drafted, needs native review
palette-develop-split-help = 元の画像と編集を並べて表示します。
# machine-drafted, needs native review
palette-develop-panel = 現像パネルへ移動
# machine-drafted, needs native review
palette-develop-panel-help = キーボード操作をパネルに移します。Esc で戻ります。
# machine-drafted, needs native review
palette-develop-speed-exposure = 露光量をスピード編集
# machine-drafted, needs native review
palette-develop-speed-exposure-help = 押し続けたままスクロールするか矢印キーを押して露光量を変更します。
# machine-drafted, needs native review
palette-develop-speed-contrast = コントラストをスピード編集
# machine-drafted, needs native review
palette-develop-speed-contrast-help = 押し続けたままスクロールするか矢印キーを押してコントラストを変更します。
# machine-drafted, needs native review
palette-develop-speed-highlights = ハイライトをスピード編集
# machine-drafted, needs native review
palette-develop-speed-highlights-help = 押し続けたままスクロールするか矢印キーを押してハイライトを変更します。
# machine-drafted, needs native review
palette-develop-speed-shadows = シャドウをスピード編集
# machine-drafted, needs native review
palette-develop-speed-shadows-help = 押し続けたままスクロールするか矢印キーを押してシャドウを変更します。
# machine-drafted, needs native review
palette-develop-speed-whites = 白レベルをスピード編集
# machine-drafted, needs native review
palette-develop-speed-whites-help = 押し続けたままスクロールするか矢印キーを押して白レベルを変更します。
# machine-drafted, needs native review
palette-develop-speed-blacks = 黒レベルをスピード編集
# machine-drafted, needs native review
palette-develop-speed-blacks-help = 押し続けたままスクロールするか矢印キーを押して黒レベルを変更します。
# machine-drafted, needs native review
palette-develop-speed-temperature = 色温度をスピード編集
# machine-drafted, needs native review
palette-develop-speed-temperature-help = 押し続けたままスクロールするか矢印キーを押してホワイトバランスの色温度を変更します。
# machine-drafted, needs native review
palette-develop-speed-vibrance = 自然な彩度をスピード編集
# machine-drafted, needs native review
palette-develop-speed-vibrance-help = 押し続けたままスクロールするか矢印キーを押して自然な彩度を変更します。
# machine-drafted, needs native review
palette-develop-crop = 切り抜きと角度補正
# machine-drafted, needs native review
palette-develop-crop-help = 写真の切り抜き、角度補正、回転、反転を行います。Enter で確定、Esc で戻ります。
# machine-drafted, needs native review
disabled-not-photo = 静止画のみ
# machine-drafted, needs native review
disabled-develop-off = 医療と科学のプロファイルでは現像はオフです
# machine-drafted, needs native review
develop-module-corrections = 補正
# machine-drafted, needs native review
develop-module-raw = RAW
# machine-drafted, needs native review
develop-off = オフ
# machine-drafted, needs native review
develop-on = オン
# machine-drafted, needs native review
develop-highlight-method = 白飛び
# machine-drafted, needs native review
develop-highlight-opposed = 色を復元
# machine-drafted, needs native review
develop-highlight-rebuild = 広い領域を復元
# machine-drafted, needs native review
develop-highlight-clip = 白にクリップ
# machine-drafted, needs native review
develop-highlight-threshold = クリップレベル
# machine-drafted, needs native review
develop-module-hot-pixels = ホットピクセル
# machine-drafted, needs native review
develop-hot-threshold = ホットピクセルのしきい値
# machine-drafted, needs native review
develop-module-ca = 色収差
# machine-drafted, needs native review
develop-ca-mode = 色にじみ
# machine-drafted, needs native review
develop-ca-auto = 自動
# machine-drafted, needs native review
develop-ca-manual = 手動
# machine-drafted, needs native review
develop-ca-red = 赤のずれ
# machine-drafted, needs native review
develop-ca-blue = 青のずれ
# machine-drafted, needs native review
develop-module-defringe = フリンジ除去
# machine-drafted, needs native review
develop-defringe = フリンジ除去
# machine-drafted, needs native review
develop-defringe-purple = 紫の色相
# machine-drafted, needs native review
develop-defringe-green = 緑の色相
# machine-drafted, needs native review
develop-defringe-range = 色相範囲
# machine-drafted, needs native review
develop-defringe-edge = エッジのコントラスト
# machine-drafted, needs native review
develop-module-flat-field = キャリブレーション画像
# machine-drafted, needs native review
develop-calibration = キャリブレーション
# machine-drafted, needs native review
develop-calibration-auto = フォルダーから
# machine-drafted, needs native review
develop-calibration-chosen = 選んだファイル
# machine-drafted, needs native review
develop-flat-choose = フラット画像を選択…
# machine-drafted, needs native review
develop-dark-choose = ダーク画像を選択…
# machine-drafted, needs native review
develop-flat-file = フラット：{ $name }
# machine-drafted, needs native review
develop-dark-file = ダーク：{ $name }
# machine-drafted, needs native review
develop-flat-use = フラット
# machine-drafted, needs native review
develop-flat-use-full = 周辺減光とほこり
# machine-drafted, needs native review
develop-flat-use-vignetting = 周辺減光のみ
# machine-drafted, needs native review
develop-flat-dialog = フラット画像を選択
# machine-drafted, needs native review
develop-dark-dialog = ダーク画像を選択
# machine-drafted, needs native review
develop-raw-kind = RAW ファイル
# machine-drafted, needs native review
develop-flat-frame = フラット画像
# machine-drafted, needs native review
develop-dark-frame = ダーク画像

# machine-drafted, needs native review
# machine-drafted, needs native review
develop-mask-add = マスクを追加
# machine-drafted, needs native review
develop-mask-kind-ellipse = 楕円
# machine-drafted, needs native review
develop-mask-kind-rectangle = 長方形
# machine-drafted, needs native review
develop-mask-kind-polygon = 多角形
# machine-drafted, needs native review
develop-mask-kind-linear = 線形グラデーション
# machine-drafted, needs native review
develop-mask-kind-radial = 円形グラデーション
# machine-drafted, needs native review
develop-mask-kind-brush = ブラシ
# machine-drafted, needs native review
develop-mask-kind-luminance = 輝度範囲
# machine-drafted, needs native review
develop-mask-kind-hue = 色相範囲
# machine-drafted, needs native review
develop-mask-kind-chroma = 彩度範囲
# machine-drafted, needs native review
develop-mask-kind-colour = カラー範囲
# machine-drafted, needs native review
develop-mask-kind-other = その他のマスク
# machine-drafted, needs native review
develop-mask-row = { $kind } { $n }
# machine-drafted, needs native review
develop-mask-mode = 描画モード
# machine-drafted, needs native review
develop-blend-normal = 通常
# machine-drafted, needs native review
develop-blend-multiply = 乗算
# machine-drafted, needs native review
develop-blend-screen = スクリーン
# machine-drafted, needs native review
develop-blend-overlay = オーバーレイ
# machine-drafted, needs native review
develop-blend-soft-light = ソフトライト
# machine-drafted, needs native review
develop-blend-lightness = 明度のみ
# machine-drafted, needs native review
develop-blend-colour = カラーのみ
# machine-drafted, needs native review
develop-mask-amount = マスクの強さ
# machine-drafted, needs native review
develop-mask-invert = マスクを反転
# machine-drafted, needs native review
develop-mask-off = オフ
# machine-drafted, needs native review
develop-mask-on = オン
# machine-drafted, needs native review
develop-mask-refine = 境界を調整
# machine-drafted, needs native review
develop-mask-show = マスクを表示
# machine-drafted, needs native review
develop-mask-show-red = 赤
# machine-drafted, needs native review
develop-mask-show-grey = グレー
# machine-drafted, needs native review
develop-mask-remove = マスクを削除
# machine-drafted, needs native review
develop-mask-combine-add = 追加
# machine-drafted, needs native review
develop-mask-combine-subtract = 減算
# machine-drafted, needs native review
develop-mask-combine-intersect = 交差
# machine-drafted, needs native review
develop-mask-row-opacity = 不透明度
# machine-drafted, needs native review
develop-mask-row-invert = 反転
# machine-drafted, needs native review
develop-mask-feather = ぼかし
# machine-drafted, needs native review
develop-mask-roundness = 角の丸み
# machine-drafted, needs native review
develop-mask-brush-size = ブラシのサイズ
# machine-drafted, needs native review
develop-mask-brush-feather = ブラシのぼかし
# machine-drafted, needs native review
develop-mask-brush-flow = 流量
# machine-drafted, needs native review
develop-mask-brush-mode = ブラシ
# machine-drafted, needs native review
develop-mask-brush-paint = 描画
# machine-drafted, needs native review
develop-mask-brush-erase = 消去
# machine-drafted, needs native review
develop-mask-range-from = 開始
# machine-drafted, needs native review
develop-mask-range-full-from = 全開始
# machine-drafted, needs native review
develop-mask-range-full-to = 全終了
# machine-drafted, needs native review
develop-mask-range-to = 終了
# machine-drafted, needs native review
develop-mask-range-test = 判定
# machine-drafted, needs native review
develop-mask-range-input = 入力
# machine-drafted, needs native review
develop-mask-range-output = 出力
# machine-drafted, needs native review
develop-mask-colour-pick = 色を選ぶ
# machine-drafted, needs native review
develop-mask-colour-pick-more = 色をもう一つ選ぶ
# machine-drafted, needs native review
develop-mask-colour-range = 範囲
# machine-drafted, needs native review
develop-mask-colour-falloff = 減衰
# machine-drafted, needs native review
develop-mask-edit = 画像上で編集
# machine-drafted, needs native review
develop-mask-row-remove = { $name } を削除
# machine-drafted, needs native review
develop-mask-remove-confirm = { $module } のマスクを削除しますか？形状は消えます。ディスク上の写真は残ります。
# machine-drafted, needs native review
develop-mask-row-remove-confirm = { $module } から { $name } を削除しますか？ほかのマスクとディスク上の写真は残ります。
# machine-drafted, needs native review
develop-mask-remove-ok = 削除
# machine-drafted, needs native review
develop-mask-removed = マスクを削除しました。
# machine-drafted, needs native review
develop-mask-added = { $name } を追加しました。
# machine-drafted, needs native review
develop-mask-full = このマスクにはこれ以上部品を追加できません。
# machine-drafted, needs native review
develop-mask-editing = { $name } を編集中。ハンドルをドラッグするか、Tab と矢印キーを使います。
# machine-drafted, needs native review
develop-mask-painting = { $name } を描画中。画像上をドラッグします。
# machine-drafted, needs native review
develop-mask-picking = 画像をクリックして色を選びます。
# machine-drafted, needs native review
develop-mask-picked = 色を選びました。
# machine-drafted, needs native review
develop-mask-done = マスクの編集を終えました。
# machine-drafted, needs native review
develop-mask-handles = マスクのハンドル
# machine-drafted, needs native review
develop-handle-centre = 中心
# machine-drafted, needs native review
develop-handle-width = 幅
# machine-drafted, needs native review
develop-handle-height = 高さ
# machine-drafted, needs native review
develop-handle-rotate = 回転
# machine-drafted, needs native review
develop-handle-feather = ぼかし
# machine-drafted, needs native review
develop-handle-inner = 内側の境界
# machine-drafted, needs native review
develop-handle-start = 始点
# machine-drafted, needs native review
develop-handle-end = 終点
# machine-drafted, needs native review
develop-handle-point = 点 { $n }
# machine-drafted, needs native review
develop-instance = レイヤー
# machine-drafted, needs native review
develop-instance-n = { $n }
# machine-drafted, needs native review
develop-instance-chosen = レイヤー { $n }。
# machine-drafted, needs native review
develop-instance-new = 新しいレイヤー
# machine-drafted, needs native review
develop-instance-duplicate = レイヤーを複製
# machine-drafted, needs native review
develop-instance-delete = レイヤーを削除
# machine-drafted, needs native review
develop-instance-earlier = 前へ移動
# machine-drafted, needs native review
develop-instance-later = 後ろへ移動
# machine-drafted, needs native review
develop-instance-delete-confirm = { $module } のレイヤー { $n } を削除しますか？設定とマスクは消えます。ディスク上の写真は残ります。
# machine-drafted, needs native review
develop-instance-delete-ok = 削除
# machine-drafted, needs native review
develop-instance-deleted = レイヤーを削除しました。
# machine-drafted, needs native review
develop-history-mask = マスク: { $module }
# machine-drafted, needs native review
develop-history-mask-shapes = マスクの形状: { $module }
# machine-drafted, needs native review
develop-history-instance-new = 新規: { $module }
# machine-drafted, needs native review
develop-history-instance-delete = 削除: { $module }
# machine-drafted, needs native review
develop-history-instance-move = 順序: { $module }
# Other tools' edits and sidecar conflicts.
# machine-drafted, needs native review
develop-others-title = ほかのアプリ
# machine-drafted, needs native review
develop-darktable-edited = darktable で編集済み:
# machine-drafted, needs native review
develop-darktable-open = darktable で開く
# machine-drafted, needs native review
develop-darktable-opening = darktable で開いています
# machine-drafted, needs native review
develop-darktable-failed = darktable を起動できませんでした: { $reason }
# machine-drafted, needs native review
develop-lightroom-import = Lightroom から読み込む
# machine-drafted, needs native review
develop-lightroom-imported = { $count ->
   *[other] Lightroom の設定を { $count } 件読み込みました。結果は近似です。
}
# machine-drafted, needs native review
develop-lightroom-nothing = 読み込むものはありません。これらの設定はここでは何も変えません。
# machine-drafted, needs native review
develop-history-lightroom = Lightroom から読み込み（近似）
# machine-drafted, needs native review
develop-conflict-title = { $file } の編集が Specular の外で変わりました
# machine-drafted, needs native review
develop-conflict-file = サイドカー: { $time } に保存
# machine-drafted, needs native review
develop-conflict-library = ライブラリ: { $time } に保存
# machine-drafted, needs native review
develop-conflict-writer = サイドカーを書いたのは { $writer } です。
# machine-drafted, needs native review
develop-conflict-detail = どちらかを残します。もう一方は置き換えられます。
# machine-drafted, needs native review
develop-conflict-use-file = ファイルの方を使う
# machine-drafted, needs native review
develop-conflict-keep-library = ライブラリの方を残す
# machine-drafted, needs native review
develop-conflict-used-file = サイドカーの編集を使います
# machine-drafted, needs native review
develop-conflict-kept-library = ライブラリの編集を残します
# machine-drafted, needs native review
toast-develop-edit-newer-process = この編集は新しい Specular で作られました。ここでは違って見えることがあります。
# machine-drafted, needs native review
info-darktable-edited = darktable で編集済み
# machine-drafted, needs native review
info-darktable-no-modules = オンのモジュールなし
# machine-drafted, needs native review
info-darktable-group = ファイル
# machine-drafted, needs native review
toast-develop-edit-partial = この編集は新しい Specular で作られました。このバージョンで表示できない部分は省いています。ここでの変更は保存されません。
# machine-drafted, needs native review
toast-develop-sidecar-unreadable = 写真の横の編集ファイルを読み込めません。そのまま残します。ここでの変更は保存されません。
# machine-drafted, needs native review
develop-demosaic = RAW ディテール
develop-demosaic-directional = 標準
develop-demosaic-dual = 平坦部を滑らかに
develop-demosaic-fast = 高速
develop-dual-threshold = 平坦部のコントラスト
