# machine-drafted, needs native review

## Model checks on IFC building models (mesh mode, Alt+Q): IDS, clashes, changes between
## versions, quantities. IFC class names (IfcWall), property set names and values are data
## and stay as the file has them. Keys named here are the defaults.

# The panel's tabs.
checks-tab = { $tab ->
    [ids] IDS
    [clashes] 干渉
    [changes] 変更
   *[quantities] 数量
}
# $tab: the tab's name.
checks-title = モデルチェック · { $tab }

# Short texts, by $what.
checks-note = { $what ->
    [not-ifc] モデルチェックには IFC モデルが必要です。
    [reading] モデルの情報を読み込み中…
    [ids-empty] IDS ファイルがありません。O キーで開きます。
    [ids-checking] チェック中…
    [ids-no-failures] 不合格はありません。
    [ids-hints] Enter 不合格・移動 · Backspace 戻る · O 開く · P プロパティ · Ctrl+S BCF · Ctrl+E CSV · Tab 次へ
    [unnamed] 名前のない仕様
    [clash-running] 干渉を検出中…
    [clash-empty] R キーで干渉を検出します。
    [clash-none] 干渉はありません。
    [clashes-hints] R 実行 · A B セット · M ハードまたはクリアランス · + − 距離 · Enter 移動 · S ステータス · Ctrl+S BCF · Ctrl+E CSV · Tab 次へ
    [changes-reading] 別のバージョンを読み込み中…
    [changes-empty] O キーでこのモデルの別のバージョンを選びます。
    [changes-none] 変更はありません。
    [changes-hints] O 別のバージョン · F フィルター · Enter 移動 · P プロパティ · Ctrl+S BCF · Ctrl+E CSV · Tab 次へ
    [quantities-hints] G グループ化 · Enter 移動 · P プロパティ · Ctrl+E CSV · Ctrl+S XLSX · Tab 次へ
   *[not-drawn] この要素には表示できる形状がありません。
}

# What a requirement is about. $name: an attribute name, an IFC class or a pattern.
checks-facet = { $facet ->
    [entity] クラス
    [attribute] 属性 { $name }
    [classification] 分類
    [material] 材料
   *[partof] { $name } の一部
}

# Why an element fails a requirement. $what: what the requirement is about; $actual: what
# the element has.
checks-reason = { $why ->
    [class] クラスが { $actual }
    [predefined] 定義済みタイプが { $actual }
    [missing] { $what } がありません
    [empty] { $what } が空です
    [value] { $what } が { $actual }: 許可されていません
    [datatype] { $what } のデータ型が { $actual }
    [system] { $what }: 体系が { $actual }
    [whole] { $what }: { $actual } の一部
    [prohibited] { $what } があってはなりません
   *[unsupported] { $what } はチェックできません
}

# Shown for a missing value.
checks-none = —

# A fact about the chosen element. $value: its value.
checks-prop = { $what ->
    [storey] 階: { $value }
    [type] タイプ: { $value }
    [material] 材料: { $value }
    [container] コンテナー: { $value }
   *[classification] 分類: { $value }
}

# $file: the IDS file; $ok: specifications that hold; $count: all of them.
checks-ids-summary = { $file }: { $count } 件中 { $ok } 件を満たす

# A specification's result. $applicable: objects it applies to; $failed: those failing.
checks-spec = { $state ->
    [pass] ✓ { $applicable } 件合格
    [fail] ✗ { $applicable } 件中 { $failed } 件不合格
    [none] ✗ 適用対象なし
    [none-ok] ✓ 適用対象なし
    [prohibited] ✗ あってはならないもの { $applicable } 件
   *[prohibited-ok] ✓ あってはならないものなし
}

# A side of a clash test. $name: an IFC class or a storey's name.
checks-set = { $kind ->
    [all] すべての要素
    [class] { $name }
   *[storey] 階 { $name }
}
# $a, $b: the two sides; $distance: the tolerance or clearance with its unit.
checks-clash-setup = A: { $a } · B: { $b } · { $mode ->
    [hard] ハード、深さ { $distance } 超
   *[clearance] 距離 { $distance } 未満
}
# $count: clashes found; $new: new ones; $resolved: earlier ones no longer found; $seconds: time taken.
checks-clash-count = { $count ->
   *[other] 干渉 { $count } 件
}、新規 { $new }、解決済み { $resolved } · { $seconds } 秒
# One clash. $depth: the depth or distance with its unit; $status: its status.
checks-clash = { $kind ->
    [hard] ハード { $depth }
   *[clearance] クリアランス { $depth }
} · { $status }
checks-clash-kind = { $kind ->
    [hard] ハード
   *[clearance] クリアランス
}
checks-status = { $status ->
    [new] 新規
    [active] 未解決
   *[resolved] 解決済み
}
# A BCF topic's title for a clash. $a, $b: the two elements.
checks-clash-topic = 干渉: { $a } × { $b }
checks-clash-done = { $count ->
    [0] 干渉なし
   *[other] 干渉 { $count } 件
}

# $file: the other version; counts of added, removed and changed elements; $filter: what is listed.
checks-changes-summary = { $file }: 追加 { $added } · 削除 { $removed } · 変更 { $changed }{ $filter ->
    [added] {" "}· 追加を表示中
    [removed] {" "}· 削除を表示中
    [changed] {" "}· 変更を表示中
   *[all] {""}
}
checks-change = { $kind ->
    [added] 追加
    [removed] 削除
   *[geometry] 形状
}
checks-change-fields = { $count ->
   *[other] { $count } 項目
}
# A field that changed. $name: the field; $old, $new: its values.
checks-field = { $name }: { $old } → { $new }
# $distance: how far it moved, with its unit.
checks-moved = { $distance } 移動

# $by: the grouping; $count: elements; $area, $volume: totals with units.
checks-qto-summary = 要素 { $count } 個 · { $area } · { $volume } · { $by ->
    [element] 要素別
    [class] クラス別
    [storey] 階別
   *[type] タイプ別
}
# Where quantities came from.
checks-source = { $source ->
    [file] ファイルから
    [computed] 計算値
   *[mixed] 一部計算値
}
# A sheet of the quantities workbook.
checks-group = { $by ->
    [element] 要素
    [class] クラス別
    [storey] 階別
   *[type] タイプ別
}

# Column headers of exported tables.
checks-col = { $col ->
    [spec] 仕様
    [guid] GlobalId
    [class] クラス
    [name] 名前
    [reason] 理由
    [a] A GlobalId
    [a-name] A
    [b] B GlobalId
    [b-name] B
    [kind] 種類
    [depth] 深さまたは距離 (m)
    [status] ステータス
    [x] X (m)
    [y] Y (m)
    [z] Z (m)
    [change] 変更
    [field] 項目
    [old] 旧
    [new] 新
    [type] タイプ
    [container] コンテナー
    [material] 材料
    [classification] 分類
    [storey] 階
    [count] 個数
    [length] 長さ (m)
    [area] 面積 (m²)
    [volume] 体積 (m³)
   *[source] 出所
}

# $reason: why, in plain words.
checks-error = 実行できません: { $reason }
# A results summary for screen readers. $failed: specifications that do not hold.
checks-ids-done = { $failed ->
    [0] チェック完了: すべての仕様を満たしています
   *[other] チェック完了: { $failed } 件の仕様を満たしていません
}
# $file: the file written.
checks-saved = { $file } を保存しました

# Several models in one scene. An element or clash set and the model file it is from.
# machine-drafted, needs native review
checks-in-model = { $element }（{ $model }）
# A clash set taken from every model of the scene.
# machine-drafted, needs native review
checks-every-model = すべてのモデル
# The model IDS, changes and quantities are about. $model: its file name.
# machine-drafted, needs native review
checks-model = モデル: { $model }
# A tab's keys in a scene of several models, by $tab (ids, clashes, changes, quantities).
# machine-drafted, needs native review
checks-hints-models = { $tab ->
    [ids] M モデル · Enter 不合格・移動 · Backspace 戻る · O 開く · P プロパティ · Ctrl+S BCF · Ctrl+E CSV · Tab 次へ
    [clashes] R 実行 · A B セット · G H それぞれのモデル · M ハードまたはクリアランス · + − 距離 · Enter 移動 · S ステータス · Ctrl+S BCF · Ctrl+E CSV · Tab 次へ
    [changes] M モデル · O 別のバージョン · F フィルター · Enter 移動 · P プロパティ · Ctrl+S BCF · Ctrl+E CSV · Tab 次へ
   *[quantities] M モデル · G グループ化 · Enter 移動 · P プロパティ · Ctrl+E CSV · Ctrl+S XLSX · Tab 次へ
}
