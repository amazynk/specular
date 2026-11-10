# التكامل العميق مع سطح المكتب: الإعدادات ← التكامل، التعيين كخلفية.

settings-section-integration = التكامل
settings-integration-default-viewer = عارض الصور الافتراضي
settings-integration-default-viewer-help = فتح الصور باستخدام { -app-name } عند النقر المزدوج عليها. تؤدي الإزالة إلى استعادة العارض السابق.
# machine-drafted, needs native review
settings-integration-extra-default-types = أنواع ملفات أخرى
settings-integration-extra-default-types-help = أنواع ملفات أخرى تُفتح باستخدام { -app-name } عند النقر المزدوج عليها، تُختار ضمن عارض الصور الافتراضي.
settings-integration-thumbnailer = الصور المصغّرة في مدير الملفات
settings-integration-thumbnailer-help = السماح لمديري الملفات بعرض معاينات للتنسيقات التي لا يستطيعون قراءتها، مثل JPEG XL وHEIC وملفات RAW للكاميرات.
settings-integration-single-instance = نسخة واحدة عبر D-Bus
settings-integration-single-instance-help = تُرسَل الملفات المفتوحة من مدير الملفات أو المشغّل أو الطرفية إلى نافذة { -app-name } المفتوحة. ينطبق على النوافذ التي تُفتح بعد ذلك.
settings-integration-file-manager-actions = إجراءات مدير الملفات
settings-integration-file-manager-actions-help = إضافة «فتح» و«مقارنة» و«تعيين كخلفية» إلى قوائم السياق في Nautilus وDolphin وThunar وNemo.
settings-integration-search-provider = بحث سطح المكتب
# machine-drafted, needs native review
settings-integration-search-provider-help = ابحث عن صور مكتبتك من بحث GNOME وKDE. تعمل الكلمات كما في شريط البحث في المكتبة.
# machine-drafted, needs native review
settings-integration-url-scheme = فتح روابط specular://
settings-integration-url-scheme-help = الروابط مثل specular:///home/me/photo.jpg في المتصفح أو في برنامج آخر تُفتح في { -app-name }. لا تُفتح إلا الملفات الموجودة على هذا الحاسوب، والملفات غير المعتادة بعد تأكيدك فقط.

integration-state-on = مفعّل
integration-state-off = معطّل
integration-state-partial = مُعدّ جزئيًا. طبّقه مرة أخرى لإصلاحه.
integration-state-unavailable = غير متاح على سطح المكتب هذا
integration-state-sandboxed = غير متاح داخل بيئة معزولة (Flatpak)
integration-state-no-bus = غير متاح: لا توجد جلسة D-Bus
integration-state-no-file-manager = غير متاح: لم يُعثر على مدير ملفات مدعوم
integration-state-no-search = غير متاح: لم يُعثر على GNOME Shell ولا KRunner
integration-state-working = جارٍ العمل…
integration-state-details = { $state } · { $details }
integration-detail-types = أنواع الصور: { $count } من { $total }
integration-detail-thumbnail-types = أنواع الصور: { $count }
integration-apply = تطبيق
integration-remove = إزالة
integration-done-on = «{ $option }» مفعّل
integration-done-off = «{ $option }» معطّل
integration-failed = تعذّر تغيير «{ $option }»: { $reason }
# machine-drafted, needs native review
integration-search-remove-ask = إيقاف بحث سطح المكتب؟
integration-search-remove-detail = سيُزال { -app-name } من بحث { $desktops }. تبقى مكتبتك وصورك كما هي.
search-provider-open-folder = فتح المجلد

integration-label-open = فتح في { -app-name }
integration-label-compare = مقارنة في { -app-name }
integration-label-wallpaper = تعيين كخلفية
integration-label-generic-name = عارض الصور
integration-label-comment = افتح الصور الكبيرة وتصفّحها فورًا

palette-set-wallpaper = تعيين كخلفية
wallpaper-set = عُيّنت الخلفية ({ $method })
wallpaper-failed = تعذّر تعيين الخلفية: { $reason }

# machine-drafted, needs native review
url-invalid = تعذّر فتح رابط { -app-name } هذا: { $reason }
url-not-found = الملفات التي يشير إليها هذا الرابط غير موجودة
url-remote-refused = يفتح { -app-name } الملفات الموجودة على هذا الحاسوب فقط، لا { $source }
url-confirm-files = { $more ->
    [0] «{ $file }»
   *[other] «{ $file }» وغيره (+{ $more })
}
url-confirm-title = { $reason ->
    [dots] فتح { url-confirm-files }؟ يصل إليه الرابط عبر مجلدات «..».
    [network] فتح { url-confirm-files }؟ إنه على مشاركة شبكة أو جهاز.
   *[hidden] فتح { url-confirm-files }؟ يحتوي اسمه على أحرف غير مرئية أو أحرف اتجاه النص.
}
url-confirm-open = فتح

# Launcher actions and recent files: machine-drafted, needs native review
integration-label-new-window = نافذة جديدة
integration-label-library = المكتبة
integration-label-slideshow = عرض شرائح للصور
settings-integration-recent-files = الملفات الأخيرة في سطح المكتب
settings-integration-recent-files-help = تظهر الصور التي تفتحها ضمن «الأخيرة» في مدير الملفات وفي قوائم الملفات الأخيرة لسطح المكتب. تؤدي الإزالة إلى حذف إدخالات { -app-name } من تلك القوائم.
integration-detail-recent = الملفات: { $count }
# machine-drafted, needs native review
integration-extra-title = أنواع ملفات أخرى
# machine-drafted, needs native review
integration-extra-intro = إلى جانب الصور، يمكن لـ { -app-name } فتح هذه الملفات عند النقر المزدوج عليها. لا يُحدَّد شيء هنا ما لم تحدده بنفسك. تبقى البرامج التي تحرر هذه الملفات في «فتح باستخدام»، وإلغاء تحديد نوع يعيده إلى البرنامج الذي كان يفتحه من قبل. وتحتاج Blackmagic RAW وDWG وSTEP وIGES وZeiss CZI أيضًا إلى برامج مجانية من صانعيها: راجع الإعدادات › مفككات الترميز الاختيارية.
integration-extra-chosen = { $count ->
    [0] لم يُختر شيء
    [zero] لم يُختر شيء
    [one] نوع واحد مختار
    [two] نوعان مختاران
    [few] { $count } أنواع مختارة
    [many] { $count } نوعًا مختارًا
   *[other] { $count } نوع مختار
}
integration-extra-count = { $on } من { $total }
integration-extra-none-yet = لا شيء بعد
integration-extra-keywords = أنواع الملفات صيغ اقتران نقر مزدوج فتح باستخدام محرر علوم طبي CAD GIS نظم المعلومات الجغرافية ثلاثي الأبعاد 3D جزيئات فيديو SVG PDF
integration-extra-group-editors = ملفات المحررات
# machine-drafted, needs native review
integration-extra-group-editors-help = مستندات برامج الرسم والتلوين: Krita وGIMP وPaint.NET وPaintShop Pro وProcreate وAffinity وOpenRaster وIllustrator وCorelDRAW وClip Studio Paint وSketch وFigma. النقر المزدوج يعرض الصورة؛ ويبقى تحريرها في البرنامج الذي أنشأها.
integration-extra-group-science = العلوم
integration-extra-group-science-help = ملفات الأجهزة وبرامج البحث: الكاميرات الحرارية والمجاهر وقياسات الأسطح وHDF5 وNetCDF والبيانات الزلزالية وبيانات الطقس.
integration-extra-group-medical = طبي
integration-extra-group-medical-help = الصور الطبية وتصوير الدماغ: DICOM وNIfTI وMINC وNRRD وتصوير المسالك العصبية وأسطح الدماغ.
integration-extra-group-cad-gis = CAD وGIS
# machine-drafted, needs native review
integration-extra-group-cad-gis-help = رسومات الراسمات والرسوم التوضيحية التقنية وتخطيطات الرقائق والخرائط: HP-GL وCGM وGDSII وshapefile وGeoPackage وKML وGPX وGeoJSON ومربعات الارتفاع. يُفتح DWG باستخدام ODA File Converter المجاني، وSTEP وIGES باستخدام OpenCASCADE.
integration-extra-group-models = ثلاثي الأبعاد والجزيئات
integration-extra-group-models-help = نماذج ثلاثية الأبعاد ونماذج المباني وسحب نقطية وجزيئات: STL وOBJ وglTF وPLY و3MF وX3D وIFC وLAS وE57 وPDB وCIF وغيرها.
integration-extra-group-videos = مقاطع الفيديو
# machine-drafted, needs native review
integration-extra-group-videos-help = مقاطع الفيديو: MP4 وMOV وM4V و3GP و3G2 وMKV وWebM وAVCHD (MTS وM2TS) وOGV وIVF وMXF. يُفتح Blackmagic RAW باستخدام Blackmagic RAW SDK المجاني.
integration-extra-group-other-images = SVG وصور أخرى
# machine-drafted, needs native review
integration-extra-group-other-images-help = صور ومستندات تُترك عادةً لبرامج أخرى: رسومات SVG وPDF وXPS وDjVu وEPS وWMF وEMF وVisio وOpenDocument وWordPerfect Graphics وكتب EPUB والخطوط، ورسوم متحركة مثل MNG وFLIC والمؤشرات المتحركة.
integration-extra-group-text = النصوص والجداول والترجمات
integration-extra-group-text-help = النص العادي والسجلات، وNFO وMarkdown، وجداول CSV وTSV، وRTF، وملفات الترجمة: SRT, WebVTT, ASS, SSA, MicroDVD, PGS, LRC, TTML وSCC.
