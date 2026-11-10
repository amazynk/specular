# Linked views: several DICOM series side by side, scrolling together by position, with
# reference lines, a 3D cursor, window and zoom sync, and manual alignment of priors.

# machine-drafted, needs native review
palette-compare-series = シリーズを比較 (連動ビュー)
# machine-drafted, needs native review
palette-compare-layout = 次のビューレイアウト
# machine-drafted, needs native review
palette-next-viewport = 次のビュー
# machine-drafted, needs native review
palette-prev-viewport = 前のビュー
# machine-drafted, needs native review
palette-reference-lines = 参照線の表示/非表示
# machine-drafted, needs native review
palette-sync-window = 次のウィンドウ同期
# machine-drafted, needs native review
palette-sync-zoom = サイズを揃えて一緒にズーム/パン
# machine-drafted, needs native review
palette-link-series = 次のシリーズ連動
# machine-drafted, needs native review
palette-align-series = 他のシリーズをここに位置合わせ
# machine-drafted, needs native review
palette-compare-series-help = 複数のシリーズを並べて表示し、一緒にスクロールします。
# machine-drafted, needs native review
palette-compare-layout-help = 表示するビューの数を増減します。
# machine-drafted, needs native review
palette-next-viewport-help = 次のビューをアクティブにします。
# machine-drafted, needs native review
palette-prev-viewport-help = 前のビューをアクティブにします。
# machine-drafted, needs native review
palette-reference-lines-help = アクティブなスライスが他のビューと交差する位置を描画します。
# machine-drafted, needs native review
palette-sync-window-help = ウィンドウ/レベルの変更を共有するビューを選びます (オフ、同じモダリティ、すべて)。
# machine-drafted, needs native review
palette-sync-zoom-help = すべてのビューを同じサイズで表示し、一緒にパンします。
# machine-drafted, needs native review
palette-link-series-help = このシリーズが他のシリーズに追従する方法を選びます。
# machine-drafted, needs native review
palette-align-series-help = 表示中のスライスで過去のシリーズをこのシリーズに合わせます。
# machine-drafted, needs native review
disabled-not-comparing = 先にシリーズを比較してください

# The bar above the viewports. Toggles show their state by colour; the others name it.
# machine-drafted, needs native review
linked-layout = レイアウト: { $cols } × { $rows }
# machine-drafted, needs native review
linked-lines = 参照線
# machine-drafted, needs native review
linked-zoom = ズーム同期
# machine-drafted, needs native review
linked-window-sync = ウィンドウ同期: { $mode ->
    [off] オフ
    [modality] 同じモダリティ
   *[all] すべて
}
# machine-drafted, needs native review
linked-link = 連動: { $mode ->
    [frame] 座標系 (Frame of Reference)
    [position] 患者位置
   *[off] オフ
}
# machine-drafted, needs native review
linked-align = ここに位置合わせ

# One viewport. $active is "yes" for the active one.
# machine-drafted, needs native review
linked-viewport = { $active ->
    [yes] ビュー { $n } / { $count } (アクティブ)
   *[no] ビュー { $n } / { $count }
}
# machine-drafted, needs native review
linked-empty = ここにシリーズをドラッグ
# machine-drafted, needs native review
linked-series = { $number } · { $modality } · { $description }
# machine-drafted, needs native review
linked-slice = スライス { $n } / { $count } · { $position } mm
# machine-drafted, needs native review
linked-in-gap = ギャップ内: 最も近いスライスまで { $distance } mm
# machine-drafted, needs native review
linked-outside = このシリーズの範囲外 ({ $distance } mm)
# machine-drafted, needs native review
linked-not-linked = 連動なし
# machine-drafted, needs native review
linked-aligned = 手動で位置合わせ済み ({ $offset } mm)
# machine-drafted, needs native review
linked-cursor = 3D カーソル: { $x }, { $y }, { $z } mm
# machine-drafted, needs native review
linked-chip = { $label }、{ $count } 枚

# Toasts and announcements.
# machine-drafted, needs native review
linked-opened = { $count ->
    [1] シリーズが 1 つだけです。検査ブラウザーからもう 1 つドラッグしてください
   *[other] { $count } 個のシリーズを比較中
}
# machine-drafted, needs native review
linked-no-geometry = このシリーズにはスライス位置がないため、連動できません
# machine-drafted, needs native review
linked-active = ビュー { $n }: { $series }
# machine-drafted, needs native review
linked-aligned-count = { $count ->
    [0] 位置合わせするものがありません。他のシリーズは同じ座標系を共有しています
    [1] 1 個のシリーズをアクティブなシリーズに位置合わせしました (このセッションのみ)
   *[other] { $count } 個のシリーズをアクティブなシリーズに位置合わせしました (このセッションのみ)
}
