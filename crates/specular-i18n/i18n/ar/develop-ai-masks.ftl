# machine-drafted, needs native review
# Specular — Develop's AI masks: subject, sky and object-at-click rows, and the
# Masks feature in Settings › Local AI.

develop-mask-kind-subject = الموضوع
develop-mask-kind-sky = السماء
develop-mask-kind-object = الجسم عند النقر
# An AI mask row while its network runs. $kind: subject, sky, object.
develop-ai-computing = { $kind ->
        [subject] جارٍ البحث عن الموضوع…
        [sky] جارٍ البحث عن السماء…
       *[object] جارٍ البحث عن الجسم…
    }
develop-ai-ready = جاهز
# An object row with its points ($n; a box counts as two).
develop-ai-object-clicks = عدد النقاط: { $n }
develop-ai-click-help =
    انقر الجسم.
    وAlt+نقر على ما ليس جزءًا منه.
    اسحب لرسم إطار.
# The row's model is not installed. $model is its name ("IS-Net").
develop-ai-needs =
    يحتاج إلى الإضافة { $model }.
    لا يحجب شيئًا حتى ذلك الحين.
develop-ai-failed = تعذّر إنشاء هذا القناع: { $error }
develop-ai-failed-worker = توقفت عملية أقنعة الذكاء الاصطناعي.
develop-ai-open-settings = فتح الذكاء الاصطناعي المحلي…
develop-ai-clear = مسح النقاط
develop-ai-too-many = بحد أقصى { $n } نقاط (الإطار يُحسب نقطتين).
develop-ai-box-set = تم تعيين الإطار
develop-ai-point-added = أُضيفت نقطة
develop-ai-point-removed = أُضيفت نقطة خارج الجسم
develop-ai-cleared = مُسحت النقاط
# Screen-reader names of the points on the picture. $n counts from 1.
develop-ai-point-on = النقطة { $n }، على الجسم
develop-ai-point-off = النقطة { $n }، خارج الجسم

## Settings › Local AI: the Masks feature
settings-ai-masks = أقنعة الذكاء الاصطناعي
settings-ai-masks-help = أقنعة الموضوع والسماء والأجسام في التحميض. نموذج الأجسام يأتي مع هذه الميزة؛ الموضوع والسماء إضافتان.
settings-ai-masks-size = النموذج
settings-ai-masks-size-help = EfficientSAM Ti أسرع. EfficientSAM S يتبع الحواف بدقة أكبر.
settings-ai-masks-size-recommended = الافتراضي
settings-ai-masks-size-small = EfficientSAM Ti
settings-ai-masks-size-standard = EfficientSAM S
settings-ai-masks-size-large = كبير
