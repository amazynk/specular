# The metadata editor: IPTC fields of the selected photos (gallery, Library, cull mode and
# the viewer).

palette-edit-metadata = 编辑元数据
palette-edit-metadata-help = 编辑所选照片的标题、说明、创作者、版权和位置。
metaedit-menu = 编辑元数据…
metaedit-title = 元数据
metaedit-close = 关闭元数据
metaedit-photos = { $count ->
   *[other] { $count } 张照片
}
metaedit-none-selected = 选择照片以编辑其元数据。
metaedit-reading = 正在读取元数据…
metaedit-mixed = 多个值：输入后应用到全部
metaedit-clearing = 将在所有照片上清除。按 Esc 保留原值。
metaedit-keywords-others = 部分照片还有其他关键词
metaedit-saved = 已保存 { $field }。
metaedit-cleared = 已清除 { $field }。
metaedit-write-failed = { $count ->
   *[other] { $count } 张照片的元数据未保存：{ $error }
}
metaedit-writes-sidecars = 保存在 XMP 附属文件中。
metaedit-writes-files = 保存在 JPEG、TIFF 和 DNG 文件中；其他文件使用 XMP 附属文件。

metaedit-group-templates = 模板
metaedit-group-description = 描述
metaedit-group-creator = 创作者
metaedit-group-rights = 版权
metaedit-group-shown = 画面中的位置
metaedit-group-created = 拍摄位置
metaedit-group-workflow = 工作流程
metaedit-templates-none = 还没有模板。模板可一次填好你常用的字段。

metaedit-field-title = 标题
metaedit-field-headline = 大标题
metaedit-field-description = 说明
metaedit-field-keywords = 关键词（以 ; 分隔）
metaedit-field-persons-shown = 画面中的人物（以 ; 分隔）
metaedit-field-event = 事件
metaedit-field-creator = 创作者（以 ; 分隔）
metaedit-field-creator-job-title = 创作者职位
metaedit-field-contact-address = 地址
metaedit-field-contact-city = 城市
metaedit-field-contact-region = 省/州
metaedit-field-contact-postal-code = 邮政编码
metaedit-field-contact-country = 国家/地区
metaedit-field-contact-phone = 电话
metaedit-field-contact-email = 电子邮件
metaedit-field-contact-url = 网站
metaedit-field-copyright = 版权声明
metaedit-field-credit = 署名
metaedit-field-source = 来源
metaedit-field-usage-terms = 使用条款
metaedit-field-shown-sublocation = 子位置
metaedit-field-shown-city = 城市
metaedit-field-shown-state = 省/州
metaedit-field-shown-country = 国家/地区
metaedit-field-shown-country-code = 国家/地区代码
metaedit-field-created-sublocation = 子位置
metaedit-field-created-city = 城市
metaedit-field-created-state = 省/州
metaedit-field-created-country = 国家/地区
metaedit-field-created-country-code = 国家/地区代码
metaedit-field-instructions = 指示
metaedit-field-job-id = 任务 ID

metaedit-template-new = 新建模板
metaedit-template-edit = 编辑
metaedit-template-remove = 移除
metaedit-template-done = 完成
metaedit-template-name = 模板名称
metaedit-template-default-name = 模板 { $n }
metaedit-template-title = 模板
metaedit-template-hint = 空字段保持不变。
metaedit-template-replace-keywords = 替换照片的关键词
metaedit-template-applied = { $count ->
   *[other] 已将“{ $name }”应用到 { $count } 张照片。
}
metaedit-template-remove-ask = 要移除模板“{ $name }”吗？
metaedit-template-remove-keep = 照片会保留其元数据。
metaedit-template-remove-yes = 移除
metaedit-template-removed = 已移除模板“{ $name }”。

app-info-edit-metadata = 编辑元数据…

settings-gallery-metadata-into-files = 将元数据写入图像文件
settings-gallery-metadata-into-files-help = 元数据编辑器也会写入 JPEG、TIFF 和 DNG 文件，不改动其中的其他内容。关闭时：仅使用 XMP 附属文件。
