# Specular — Export slideshow as video (DESIGN.md §6.5). Arabic. Machine-drafted, needs native review.

palette-export-slideshow = تصدير عرض الشرائح كفيديو…

video-export-title = تصدير عرض الشرائح كفيديو
video-export-resolution = الدقة
# $width and $height in pixels.
video-export-resolution-window = النافذة ({ $width } × { $height })
video-export-fps = معدل الإطارات
# $fps frames per second, a number.
video-export-fps-value = { $fps } إطار/ث
video-export-quality = الجودة
video-export-quality-high = عالية
video-export-quality-standard = قياسية
video-export-quality-compact = مضغوطة
video-export-images = { $count ->
    [zero] لا صور
    [one] صورة واحدة، بترتيب عرض الشرائح وانتقاله وتسمياته التوضيحية
    [two] صورتان، بترتيب عرض الشرائح وانتقاله وتسمياته التوضيحية
    [few] { $count } صور، بترتيب عرض الشرائح وانتقاله وتسمياته التوضيحية
    [many] { $count } صورة، بترتيب عرض الشرائح وانتقاله وتسمياته التوضيحية
   *[other] { $count } صورة، بترتيب عرض الشرائح وانتقاله وتسمياته التوضيحية
}
# $length a duration ("2:40"), $size a file size ("38 MB").
video-export-summary = المدة نحو { $length }، والحجم نحو { $size }
video-export-export = تصدير…
video-export-replace = استبدال
# $file a file name.
video-export-replace-question = { $file } موجود بالفعل. هل تريد استبداله؟
# $percent a formatted percentage ("42%").
video-export-progress = جارٍ التصدير… { $percent }
video-export-cancelling = جارٍ الإلغاء…
# $file a file name, $size a file size ("38 MB").
video-export-saved = تم حفظ { $file } ({ $size })
video-export-skipped = { $count ->
    [zero] لم تُستبعد أي صورة.
    [one] تعذّرت قراءة صورة واحدة فاستُبعدت.
    [two] تعذّرت قراءة صورتين فاستُبعدتا.
    [few] تعذّرت قراءة { $count } صور فاستُبعدت.
    [many] تعذّرت قراءة { $count } صورة فاستُبعدت.
   *[other] تعذّرت قراءة { $count } صورة فاستُبعدت.
}
video-export-failed = تعذّر تصدير الفيديو: { $reason }
video-export-cancelled = أُلغي التصدير
video-export-no-images = تعذّرت قراءة جميع الصور.
video-export-show-in-folder = إظهار في المجلد

## Toolbar item and compatibility (machine-drafted, needs native review)
settings-viewing-toolbar-items-export-slideshow = تصدير عرض الشرائح كفيديو
video-export-compatibility = التوافق
video-export-compatibility-current = المشغلات الحالية
video-export-compatibility-all = كل المشغلات
