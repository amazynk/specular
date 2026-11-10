# machine-drafted, needs native review
## 3D モデルのレビュー: ウォークとフライ、断面と断面ボックス、保存したビュー、BCF トピック
## (3D モード)。記載のキーは既定のキーです。

cheatsheet-review = レビュー

palette-mesh-walk = ウォーク
palette-mesh-fly = フライ
palette-mesh-views = 保存したビュー
palette-mesh-save-view = ビューを保存
palette-mesh-section-plane = 断面を追加
palette-mesh-section-box = 断面ボックス
palette-mesh-section-flip = 断面を反転
palette-mesh-section-clear = すべての断面を削除
palette-mesh-measure-cut = 断面上で計測
palette-mesh-bcf-topics = BCF トピック
# machine-drafted, needs native review
palette-mesh-model-checks = モデルチェック
# machine-drafted, needs native review
palette-mesh-models = シーン内のモデル
# machine-drafted, needs native review
palette-mesh-add-model = モデルを追加…
# machine-drafted, needs native review
palette-mesh-add-model-folder = フォルダーからモデルを追加…

# ウォークとフライ: 通知と表示行、$what ごと。$eye: 床からの目の高さ (単位付き、walk-line のみ)。
mesh-nav = { $what ->
    [walk-toast] ウォーク中: W A S D で移動、ドラッグで見回す、Esc で終了
    [fly-toast] フライ中: W A S D で移動、Q E で下降と上昇、Esc で終了
    [orbit] オービット
    [walk-line] ウォーク中 · 目の高さ 床から { $eye }
    [walk-keys] W A S D 移動 · ドラッグ 見回す · Shift 高速 · Esc 終了
    [preparing] 壁の準備がまだです: 壁を通り抜けます
    [fly-keys] W A S D 移動 · Q E 下降、上昇 · ドラッグ 見回す · Shift 高速 · Esc 終了
   *[fly-line] フライ中
}

# 断面、計測、保存したビューの短いテキスト、$what ごと。
bim-note = { $what ->
    [plane-added] 断面を追加しました: ハンドルをドラッグして移動します
    [box-on] 断面ボックス オン: 面のハンドルをドラッグして移動します
    [box-off] 断面ボックス オフ
    [cleared] 断面を削除しました
    [measure-on] 断面上の 2 点をクリックします
    [measure-needs-cut] 先に断面か断面ボックスを追加してください
    [measure-off-cut] 断面上をクリックしてください
    [measure-hint] 計測中: 断面上の 2 点をクリックします
    [ask-view-name] ビューの名前
    [ask-rename] 新しい名前
    [views-empty] 保存したビューはありません。N キーでこのビューを保存します。
    [views-hints] Enter 移動 · N 保存 · F2 名前の変更 · Delete 削除 · P PNG として保存
    [delete-detail] 削除されるのは保存したビューだけです。モデルは変更されません。
    [delete-ok] 削除
    [not-over-model] 別のファイルを選んでください: このファイルはモデルを置き換えてしまいます。
   *[view-name] ビュー
}

# BCF (BIM Collaboration Format) の短いテキスト、$what ごと。
bcf-note = { $what ->
    [title-none] BCF トピック
    [new-file] 新規
    [empty] トピックはありません。O キーで BCF ファイルを開くか、N キーでトピックを追加します。
    [hints] Enter 移動 · ← → ビューポイント · N トピック · C コメント · S ステータス · E タイトル · O 開く · Ctrl+S エクスポート
    [ask-title] トピックのタイトル
    [ask-comment] コメント
    [untitled] 無題
    [topic-added] このビュー付きでトピックを追加しました
   *[comment-added] このビュー付きでコメントを追加しました
}

# $planes: 断面の数。
bim-sections-line = { $planes ->
    [0] 断面なし
   *[other] 断面 { $planes } 個
}
# $size: 断面ボックスの大きさ (単位付き)。
bim-box-line = ボックス { $size }
# $count: 断面の最大数。
bim-planes-full = 断面は最大 { $count } 個です
# スクリーン リーダー用のドラッグ ハンドル。$n: その番号。
bim-handle = { $kind ->
    [box] 断面ボックスの面 { $n }
   *[plane] 断面 { $n }
}
# $length: 単位付きの長さ。
bim-measure-line = 断面上の長さ: { $length }
bim-views-title = { $count ->
   *[other] 保存したビュー { $count } 件
}
# $name: ビューの名前。
bim-view-saved = 「{ $name }」を保存しました
bim-view-delete-title = ビュー「{ $name }」を削除しますか？
# $file: 書き出したファイル。
bim-saved-file = { $file } を保存しました
# $reason: 失敗した理由。
bim-failed = { $what ->
    [open] BCF ファイルを開けませんでした: { $reason }
    [views] ビューはこのセッションの間だけ保持されます: { $reason }
   *[save] 保存できませんでした: { $reason }
}
# $count: トピック数; $file: アーカイブのファイル名。
bcf-title = { $count ->
   *[other] トピック { $count } 件
} · { $file }
bcf-comments = { $count ->
   *[other] コメント { $count } 件
}
# $at: 表示中のビューポイント; $count: トピックのビューポイント数。
bcf-viewpoint-of = ビューポイント { $at } / { $count }
bcf-opened = { $count ->
   *[other] トピック { $count } 件を開きました
}

## Several IFC models in one scene (the Models list, Alt+Shift+M).

# The list's title. $count: models in the scene.
# machine-drafted, needs native review
fed-title = { $count ->
   *[other] モデル: { $count }
}
# machine-drafted, needs native review
fed-note = { $what ->
    [not-ifc] このファイルは IFC モデルではありません。
    [not-ifc-open] まず IFC モデルを開いてください。組み合わせられるのは IFC モデルだけです。
    [this-model] このモデル
    [hints] A モデルを追加 · Shift+A フォルダーを追加 · Space 表示・非表示 · C モデル別に色分け · Enter 移動 · Delete 外す
    [origin-ok] 原点に配置
    [first-stays] 開いたモデルはそのまま残ります。変更するには別のファイルを開いてください。
    [remove-detail] シーンから外れます。ディスク上のファイルは残ります。
   *[remove-ok] 外す
}
# A model's row. $basis: how it was placed; $elements: its elements; $shown: shown or hidden.
# machine-drafted, needs native review
fed-row = { $basis } · { $elements ->
   *[other] 要素 { $elements } 個
} · { $shown ->
    [hidden] 非表示
   *[shown] 表示
}
# How a model was placed against the first. $crs: the coordinate system's name.
# machine-drafted, needs native review
fed-basis = { $basis ->
    [reference] 開いたモデル
    [map-crs] 地図座標で配置（{ $crs }）
    [map] 地図座標で配置
    [site] 敷地の緯度・経度で配置（概算）
    [project] 共有プロジェクト座標で配置
   *[origin] 原点に配置（位置合わせなし）
}
# Why a model cannot be aligned. $first, $this: coordinate system names.
# machine-drafted, needs native review
fed-unaligned = { $why ->
    [crs] モデルの座標系が異なります: { $first } と { $this }。
    [only-first] 開いたモデルはジオリファレンス済みですが、このモデルは違います。
    [only-this] このモデルはジオリファレンス済みですが、開いたモデルは違います。
   *[broken] このモデルの地図変換は使用できません（縮尺がゼロです）。
}
# The chosen model's placement. $distance: how far it moved; $angle: degrees turned.
# machine-drafted, needs native review
fed-placed = { $distance } 移動、{ $angle }° 回転
# machine-drafted, needs native review
fed-adding = { $count ->
   *[other] { $count } 個のモデルを読み込み中…
}
# machine-drafted, needs native review
fed-ask-origin = { $file } は位置合わせできません。原点に配置しますか？
# machine-drafted, needs native review
fed-no-files = { $folder } に IFC ファイルがありません。
# machine-drafted, needs native review
fed-not-added = { $file } は追加されませんでした: { $reason }
# machine-drafted, needs native review
fed-added = { $file } を追加しました。シーン内のモデル: { $count }
# machine-drafted, needs native review
fed-remove-title = { $file } をシーンから外しますか？
# machine-drafted, needs native review
fed-removed = { $file } をシーンから外しました。
# machine-drafted, needs native review
fed-shown = { $file } { $shown ->
    [hidden] を非表示にしました
   *[shown] を表示しました
}
# machine-drafted, needs native review
fed-colours = { $on ->
    [on] モデル別の色分け: オン
   *[off] モデル別の色分け: オフ
}

## PMI (product manufacturing information) of a CAD model: dimensions, geometric tolerances,
## datums and the file's saved views (mesh mode).

# machine-drafted, needs native review
palette-mesh-pmi = PMI と保存したビュー
# $count: annotations in the model.
# machine-drafted, needs native review
pmi-title = { $count ->
   *[other] 注釈 { $count } 件
}
# machine-drafted, needs native review
pmi-view = 保存したビュー
# machine-drafted, needs native review
pmi-view-all = すべての注釈を表示
# $count: annotations the saved view shows.
# machine-drafted, needs native review
pmi-view-shows = { $count ->
   *[other] 注釈 { $count } 件を表示
}
# $name: the saved view's name.
# machine-drafted, needs native review
pmi-view-shown = ビュー「{ $name }」を表示中。A ですべて表示。
# machine-drafted, needs native review
pmi-has-faces = 面をハイライト
# Short texts, by $what.
# machine-drafted, needs native review
pmi-note = { $what ->
    [none] このモデルには PMI がありません。
    [hints] ↑ ↓ 選択 · Enter 移動 · A すべて · D 図示どおり · Shift+A 非表示
    [hidden] 注釈を非表示
    [shown] 注釈を表示
    [as-drawn] 注釈をファイルの描画どおりに表示
    [as-labels] 注釈をラベルで表示
   *[all-shown] すべての注釈を表示
}
# What an annotation is, by $what.
# machine-drafted, needs native review
pmi-kind = { $what ->
    [dimension] 寸法
    [tolerance] 幾何公差
    [datum] データム
    [target] データムターゲット
   *[graphic] 図形注記
}
# machine-drafted, needs native review
pmi-dimension = { $what ->
    [linear] 距離
    [angular] 角度
    [diameter] 直径
    [radius] 半径
    [length] 長さ
    [thickness] 厚さ
   *[other] 寸法
}
# Geometric characteristics (ISO 1101), by $what.
# machine-drafted, needs native review
pmi-characteristic = { $what ->
    [straightness] 真直度
    [flatness] 平面度
    [circularity] 真円度
    [cylindricity] 円筒度
    [line-profile] 線の輪郭度
    [surface-profile] 面の輪郭度
    [angularity] 傾斜度
    [perpendicularity] 直角度
    [parallelism] 平行度
    [position] 位置度
    [concentricity] 同心度
    [coaxiality] 同軸度
    [symmetry] 対称度
    [circular-runout] 円周振れ
   *[total-runout] 全振れ
}
