# アニメーション再生と閲覧モード（アーカイブ、コミック）。

disabled-not-animated = この画像はアニメーションではありません
disabled-in-archive = アーカイブ内のページはゴミ箱に移動できません

## コマンドパレットとショートカット一覧

palette-play-pause = アニメーションを再生／一時停止
palette-next-frame = 次のフレーム
palette-prev-frame = 前のフレーム
palette-faster = 速く再生
palette-slower = 遅く再生
palette-normal-speed = 通常の再生速度
palette-loop-forever = アニメーションを無限に繰り返す
palette-right-to-left = 右から左へのページ順
# machine-drafted, needs native review
palette-contents = 目次
palette-spreads = 見開き表示

## アニメーションのコントロール

transport-region = アニメーションのコントロール
transport-play = 再生
transport-pause = 一時停止
transport-prev-frame = 前のフレーム
transport-next-frame = 次のフレーム
transport-slower = 遅く
transport-faster = 速く
transport-frame = フレーム { $index } / { $count }
transport-frame-unknown = フレーム { $index }
transport-speed = { $speed }×
transport-speed-name = 再生速度 { $speed }×
# machine-drafted, needs native review
motion-badge = モーション
motion-badge-live = Live
motion-play = モーションクリップを再生
motion-play-live = Live Photo を再生
motion-stop = 静止画に戻る
toast-motion-failed = モーションクリップを再生できません: { $reason }

## 通知

toast-speed = 再生速度 { $speed }×
toast-loop-forever-on = 無限に繰り返します
toast-loop-forever-off = ファイルの指定どおり繰り返します
toast-right-to-left-on = 右から左へ読みます
toast-right-to-left-off = 左から右へ読みます
toast-spreads-on = 見開き表示
toast-spreads-off = 単ページ表示

## アーカイブ

title-archive-page = { $page } — { $archive }
readout-position-spread = { $first }–{ $last } / { $count }
archive-needs-sandbox = { $format } アーカイブはサンドボックス化されたデコーダーでのみ開けますが、このビルドにはまだ含まれていません。
archive-empty = このアーカイブには画像がありません。
archive-failed = アーカイブを開けませんでした: { $error }
archive-encrypted = このアーカイブは暗号化されています。パスワードを入力するには、もう一度開いてください。
archive-wrong-password = このアーカイブのパスワードが正しくありません。
password-title = パスワードが必要です
password-archive = 「{ $archive }」は暗号化されています。
password-label = パスワード
password-show = パスワードを表示
password-wrong = パスワードが正しくありません。もう一度お試しください。
password-open = 開く
password-cancel = キャンセル

## 設定

settings-viewing-animation-autoplay = アニメーションを自動再生
settings-viewing-animation-autoplay-help = アニメーション画像（GIF、APNG、WebP など）を開いたらすぐに再生します。
settings-viewing-animation-loop = アニメーションの繰り返し
settings-viewing-animation-loop-help = アニメーションを繰り返す回数。
settings-viewing-animation-loop-file = ファイルの指定どおり
settings-viewing-animation-loop-forever = 無限
settings-viewing-animation-loop-once = 1回だけ
settings-viewing-animation-short-delays = ごく短いフレーム間隔
settings-viewing-animation-short-delays-help = 10 ms 以下のフレーム: ウェブブラウザーと同じく 100 ms に遅くするか、保存されたとおりに再生します。
settings-viewing-animation-short-delays-browser = ウェブブラウザーと同じ（100 ms）
settings-viewing-animation-short-delays-as-stored = 保存されたとおり
settings-browsing-reading-order = アーカイブのページ順
settings-browsing-reading-order-help = ComicInfo.xml で指定されている場合（マンガ）、コミックのアーカイブは右から左へ読みます。ここで変更できます。
settings-browsing-reading-order-auto = アーカイブに従う
settings-browsing-reading-order-left-to-right = 左から右
settings-browsing-reading-order-right-to-left = 右から左
settings-browsing-page-layout = アーカイブのページ表示
settings-browsing-page-layout-help = 1ページずつ、または2ページを並べて表示します。表紙は常に単独で表示します。
settings-browsing-page-layout-auto = アーカイブに従う
settings-browsing-page-layout-single = 単ページ
settings-browsing-page-layout-spreads = 見開き

## Pages of multi-image files (ICNS sizes, texture layers, faces and slices, TIFF pages) (machine-drafted, needs native review)
palette-next-page = 次のページ
palette-prev-page = 前のページ
palette-first-page = 最初のページ
palette-last-page = 最後のページ
disabled-one-page = この画像は 1 ページだけです
page-indicator = { $index } / { $count } ページ
page-indicator-labelled = { $page }、{ $label }
page-label-icon = { $width }×{ $height }
page-label-icon-density = { $width }×{ $height } @{ $density }x
page-label-layer = レイヤー { $index }
page-label-face = 面 { $face }
page-label-sphere = スフィアマップ
page-label-slice = スライス { $index }
page-label-join = { $a } · { $b }
page-label-slide = スライド
page-label-slide-label = ラベル
page-label-slide-macro = 全体像
page-label-slide-thumbnail = サムネイル
# machine-drafted, needs native review
page-label-thermal = 熱画像
page-label-visible = 可視光写真
# machine-drafted, needs native review
page-label-left-view = 左ビュー
# machine-drafted, needs native review
page-label-right-view = 右ビュー
# machine-drafted, needs native review
# An object of a CERN ROOT file in the page picker: its path in the file and its class, e.g.
# "run1/hits (TH2D)".
page-label-object = { $name } ({ $class })
# machine-drafted, needs native review
# A view of one step of a circuit board job ("panel · Top").
page-label-board-step = { $step } · { $view }
# machine-drafted, needs native review
# A focal plane and a time point of a remote microscopy image (OMERO), counted from 1.
page-label-z = Z { $index } / { $count }
page-label-t = T { $index } / { $count }
# machine-drafted, needs native review
# Views of a remote-sensing product (an ENVI cube, a Sentinel-2 product) in the page picker.
page-label-product-composite = 既定のバンド
page-label-product-bands = すべてのバンド
page-label-product-true-colour = トゥルーカラー
page-label-product-false-colour = フォールスカラー (近赤外)
page-label-product-swir = 短波長赤外
page-label-product-agriculture = 農業
page-label-product-scene-class = シーン分類
page-label-product-cloud = 雲の確率
# A product view at a pixel size, e.g. "True colour, 10 m".
page-label-product-at = { $view }、{ $metres } m
title-file-page = { $file } — { $page }
info-page-group = ページ
info-page-position = 位置
info-page-contents = 内容

# machine-drafted, needs native review
# OpenColorIO (Settings › Colour): why scene-linear images are shown without it.
ocio-failed = OCIO は使用されていません: { $reason }
ocio-builtin-config = $OCIO は組み込みの構成 (ocio://) を指しています。ファイルとして保存し、そのファイルを選択してください。

## Page labels for icons and collections (machine-drafted, needs native review)
page-label-size = { $width }×{ $height }
page-label-size-bits = { $width }×{ $height } · { $bits } ビット

## Page labels for Photoshop layers (machine-drafted, needs native review)
page-label-merged = 統合画像
page-label-from-layers = レイヤーから合成
page-label-layer-hidden = { $name }（非表示）
page-label-layer-unnamed = 名前のないレイヤー { $index }
