# machine-drafted, needs native review
# Photographer polish pass: Develop regions, crop rows, import choices, keyword renaming, auto-apply rules.

palette-develop-next-region = المنطقة التالية
palette-develop-next-region-help = نقل لوحة المفاتيح إلى الأمام: الصورة، اللوحة، شريط الصور.
palette-develop-prev-region = المنطقة السابقة
palette-develop-prev-region-help = نقل لوحة المفاتيح إلى الخلف: شريط الصور، اللوحة، الصورة.
develop-region-picture = الصورة
develop-region-panel = اللوحة

develop-crop-aspect-row = نسبة الأبعاد
develop-crop-aspect-option-free = حرة
develop-crop-aspect-option-original = الأصلية
develop-crop-aspect-option-custom = مخصصة
develop-crop-type-ratio = كتابة نسبة…
develop-crop-swap = التبديل بين العمودي والأفقي
develop-crop-turn-left = تدوير إلى اليسار
develop-crop-turn-right = تدوير إلى اليمين
develop-crop-flip-horizontal = قلب أفقيًا
develop-crop-flip-vertical = قلب عموديًا
develop-crop-ratio-title = نسبة أبعاد القص
develop-crop-ratio-field = العرض:الارتفاع
develop-crop-ratio-ok = تعيين
develop-crop-ratio-bad = اكتب نسبة مثل 5:4، أو رقمًا واحدًا مثل 1.85.

import-template = قالب البيانات الوصفية
import-style = نمط التحميض
import-choice-none = بلا
import-choice-said = { $label }: { $value }
import-keywords = الكلمات المفتاحية
import-keywords-hint = مثل العائلة; الأماكن > باريس
import-style-missing = النمط { $name } لم يعد موجودًا. استُوردت الصور من دونه.

keywords-rename = إعادة التسمية…
keywords-move = نقل إلى…
keywords-rename-of = اسم جديد لـ{ $keyword }
keywords-move-of = وضع { $keyword } تحت (فارغ: المستوى الأعلى)
keywords-move-into-itself = لا يمكن وضع { $keyword } تحت نفسها.
keywords-renaming = جارٍ تغيير اسم { $from } إلى { $to }…
keywords-merging = جارٍ دمج { $from } في { $to }…
keywords-renamed = أصبحت { $from } الآن { $to } في { $count ->
    [zero] { $count } صورة
    [one] { $count } صورة
    [two] { $count } صورتين
    [few] { $count } صور
    [many] { $count } صورةً
   *[other] { $count } صورة
}
keywords-rename-row = الكلمة المفتاحية { $from } إلى { $to }
keywords-rename-finding = جارٍ البحث عن الصور…
keywords-rename-row-detail = { $done } من { $total } صورة

develop-split-moved = الفاصل عند { PERCENT($percent) }

develop-rules-placeholder-edit = اختر قاعدة لتغييرها أو حذفها
develop-rule-edit-placeholder = { $rule }
develop-rule-maker = الشركة المصنّعة: { $value }
develop-rule-model = الطراز: { $value }
develop-rule-lens = العدسة: { $value }
develop-rule-iso = ISO: { $value }
develop-rule-type = نوع الملف: { $value }
develop-rule-style = النمط: { $value }
develop-rule-any = أي
develop-rule-type-any = أي ملف
develop-rule-delete = حذف القاعدة…
develop-rule-field-title = تغيير «{ $rule }»
develop-rule-field-maker = الشركة المصنّعة (يكفي جزء من الاسم؛ فارغ: أي)
develop-rule-field-model = الطراز (يكفي جزء من الاسم؛ فارغ: أي)
develop-rule-field-lens = العدسة (يكفي جزء من الاسم؛ فارغ: أي)
develop-rule-field-iso = ISO، مثل 100-800 أو 1600- أو -400 (فارغ: أي)
develop-rule-field-ok = تعيين
develop-rule-iso-bad = اكتب نطاق ISO مثل 100-800.
develop-rule-style-placeholder = اختر نمط القاعدة
