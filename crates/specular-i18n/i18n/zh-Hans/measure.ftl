# Specular — 测量工具（任意图像上的长度、角度、区域及其统计）。机器草译，需母语审校。

palette-measure = 测量
palette-measure-copy-csv = 将测量复制为 CSV
palette-measure-export = 导出测量…
toast-copied-measurements = 已将测量复制为 CSV

measure-tool-title = 测量
measure-tool-list = 测量
measure-tool-count = { $count ->
    [0] 无测量
   *[other] { $count } 项测量
}
measure-tool-empty = 选择工具后在图像上绘制。

measure-tool-point = 点
measure-tool-line = 直线
measure-tool-polyline = 折线
measure-tool-angle = 角度
measure-tool-rect = 矩形
measure-tool-ellipse = 椭圆
measure-tool-polygon = 多边形
measure-tool-freehand = 手绘
measure-tool-calibrate = 校准
# machine-drafted, needs native review
measure-tool-spot = 测温点

measure-tool-undo = 撤销
measure-tool-delete = 删除
measure-tool-copy = 复制 CSV
measure-tool-export = 导出…
measure-tool-import = 导入…
measure-tool-done = 完成
measure-tool-unit = 单位：{ $unit }
measure-tool-apply = 应用

measure-tool-hint-line = 拖动或单击两个点。方向键移动十字线，空格键放置一个点。
measure-tool-hint-angle = 单击三个点，第二个为顶点。
measure-tool-hint-rect = 拖动或单击两个角。
measure-tool-hint-ellipse = 拖动或单击其外框的两个角。
measure-tool-hint-polygon = 单击各个角；按 Enter 或单击第一个角即闭合。
measure-tool-hint-calibrate = 沿已知长度的物体画一条线。
# machine-drafted, needs native review
measure-tool-hint-spot = 单击一个点读取其温度。
measure-tool-known-length = 已知长度：{ $value } { $unit }（输入数字，U 切换单位，Enter 应用）
measure-tool-calibrated = 已校准：一个像素为 { $size } { $unit }
measure-tool-bad-length = 请输入大于零的长度。

measure-tool-item = { $n }. { $shape }
measure-tool-area = 面积 { $value }
measure-tool-mean = 平均值 { $mean } ± { $sd }
measure-tool-means = 平均值 { $values }
measure-tool-range = 范围 { $min } – { $max }，共 { $count } px
measure-tool-pixels = 共 { $count } px
measure-tool-stats-pending = 正在读取像素…
measure-tool-stats-none = 此处无像素值
measure-tool-approximate = 来自缩小副本（近似值）

measure-tool-deleted = 已删除测量
measure-tool-undone = 已撤销
measure-tool-nothing = 此图像上没有测量
measure-tool-exported = 测量已保存到 { $name }
measure-tool-export-failed = 无法保存测量：{ $reason }
measure-tool-imported = { $count ->
   *[other] 已导入 { $count } 项测量。
}
measure-tool-skipped = { $count ->
   *[other] 有 { $count } 个形状无法读取。
}
measure-tool-import-failed = 无法读取测量：{ $reason }
measure-tool-too-large = 文件过大。
# machine-drafted, needs native review
measure-tool-sr-not-dicom = DICOM SR 需要 DICOM 图像。请改存为 CSV 或 GeoJSON。

measure-source-uncalibrated = 像素（未校准）
measure-source-dicom-pixel-spacing = DICOM 像素间距
measure-source-dicom-calibrated = DICOM 校准间距
measure-source-dicom-imager-pixel-spacing = DICOM 探测器处间距
measure-source-dicom-magnification = DICOM 放大率校正间距
measure-source-dicom-ultrasound-region = DICOM 超声区域
measure-source-tiff-resolution = TIFF 分辨率
measure-source-exif-resolution = EXIF 分辨率
measure-source-ome-physical-size = OME 物理尺寸
measure-source-fits-wcs = FITS 世界坐标
measure-source-vendor-tag = 厂商元数据
measure-source-drawing-units = 图纸单位
# machine-drafted, needs native review
measure-source-drawing-scale = 根据图纸比例尺
# machine-drafted, needs native review
measure-source-drawing-scale-ratio = 取自图纸（比例尺 { $scale }）
# machine-drafted, needs native review
measure-source-drawing-scale-viewport = 取自图纸（比例尺 { $scale }，视口 { $viewport }）
# machine-drafted, needs native review
measure-source-map-scale = 地图中心处的比例尺（近似值；地图工具在地球表面上测量）
measure-source-user = 您的校准
