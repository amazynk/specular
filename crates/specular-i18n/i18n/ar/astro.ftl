# machine-drafted, needs native review
## Astronomy interop: DS9 regions, plate solving, SAMP, HiPS sky surveys.

palette-science-load-regions = تراكب مناطق DS9…
palette-science-load-regions-help = رسم أشكال ملف مناطق DS9 (.reg) على الصورة.
palette-science-export-regions = حفظ المناطق بصيغة DS9…
palette-science-export-regions-help = حفظ القياسات والمناطق المتراكبة في ملف مناطق DS9.
palette-science-edit-regions = تحرير المناطق كقياسات
palette-science-edit-regions-help = تحويل المناطق المتراكبة إلى قياسات يمكن تحريكها وتغييرها.
palette-science-clear-regions = إزالة المناطق المتراكبة
palette-science-clear-regions-help = إزالة مناطق DS9 من الصورة.
palette-science-plate-solve = الحل الأسترومتري
palette-science-plate-solve-help = إيجاد الإحداثيات السماوية للصورة باستخدام ASTAP أو astrometry.net. التشغيل مرة أخرى يوقفه.
palette-science-save-plate-solution = حفظ الحل الأسترومتري
palette-science-save-plate-solution-help = حفظ الإحداثيات السماوية التي وُجدت بجانب الصورة، في ملف .wcs.
palette-science-sky-survey = مسح السماء في الخلفية
palette-science-sky-survey-help = عرض مسح السماء التالي حول الصورة، أو لا شيء.
palette-science-samp-send-image = إرسال الصورة إلى تطبيقات SAMP
palette-science-samp-send-image-help = فتح هذه الصورة في DS9 وAladin والتطبيقات الأخرى على موزّع SAMP.
palette-science-samp-send-position = إرسال الموضع إلى تطبيقات SAMP
palette-science-samp-send-position-help = توجيه التطبيقات الأخرى إلى الموضع السماوي تحت المؤشر.

disabled-samp-off = اتصل بموزّع SAMP أولًا (الإعدادات › التكامل).

settings-integration-samp = الاتصال بموزّع SAMP
settings-integration-samp-help = تبادل الصور والجداول والمواضع السماوية مع DS9 وTOPCAT وAladin وتطبيقات الفلك الأخرى. يُستخدم موزّع على هذا الحاسوب فقط.
settings-integration-astap-path = برنامج ASTAP
settings-integration-astap-path-help = الحلّال الأسترومتري ASTAP، إن لم يُعثر عليه تلقائيًا.
settings-integration-solve-field-path = برنامج astrometry.net
settings-integration-solve-field-path-help = برنامج solve-field من astrometry.net، إن لم يُعثر عليه تلقائيًا.

astro-regions-filter = مناطق DS9
astro-regions-loaded = { $name }: عدد المناطق: { $count }.
astro-regions-refused = أسطر لم تُقرأ: { $count }. أولها: السطر { $line }، { $reason }.
astro-regions-approximate = أشكال panda رُسمت بحافتيها الداخلية والخارجية: { $count }.
astro-regions-none-here = { $name }: لا تقع أي منطقة على هذه الصورة.
astro-regions-failed = تعذّرت قراءة { $name }: { $reason }
astro-regions-cleared = أُزيلت المناطق.
astro-regions-nothing = لا توجد مناطق أو قياسات لحفظها.
astro-regions-saved = حُفظ { $name }.
astro-regions-skipped = أشكال خارج الإحداثيات السماوية أُغفلت: { $count }.
astro-regions-save-failed = تعذّر حفظ المناطق: { $reason }
astro-regions-edited = مناطق أصبحت قياسات: { $count }.
astro-regions-no-wcs = تحتاج المناطق السماوية إلى صورة ذات إحداثيات سماوية.

astro-solve-no-solver = لم يُعثر على حلّال أسترومتري. ثبّت ASTAP أو astrometry.net، أو اختر واحدًا من الإعدادات › التكامل.
astro-solve-running = جارٍ الحل باستخدام { $solver }… { $seconds } ث
astro-solve-stop = إيقاف
astro-solve-stopped = أُوقف الحل الأسترومتري.
astro-solve-done = تم الحل: { $ra }، { $dec }، { $scale } لكل بكسل.
astro-solve-failed = فشل الحل الأسترومتري: { $reason }
astro-solve-not-local = لا يمكن حل إلا الصور الموجودة في مجلد على هذا الحاسوب.
astro-solve-none = لا يوجد حل أسترومتري لهذه الصورة بعد.
astro-solve-saved = حُفظ { $name }. الصورة نفسها لم تتغير.
astro-solve-sidecar = الإحداثيات السماوية من { $name }.

astro-samp-connected = تم الاتصال بموزّع SAMP.
astro-samp-no-hub = ‏SAMP: { $reason }. ستُعاد المحاولة عند بدء موزّع.
astro-samp-lost = توقف موزّع SAMP. ستُعاد المحاولة عند بدء موزّع آخر.
astro-samp-sent = أُرسل إلى التطبيقات: { $count }.
astro-samp-nobody = لا يستقبل أي تطبيق على الموزّع هذا.
astro-samp-failed = ‏SAMP: { $reason }
astro-samp-refused = ‏SAMP: { $url } ليس على هذا الحاسوب، لذا لن يُفتح.
astro-samp-pointed = وُجّهت إلى { $ra }، { $dec }.
astro-samp-off-image = { $ra }، { $dec } ليس على هذه الصورة.
astro-samp-no-table = ‏SAMP: هذا الجدول غير متراكب هنا.
astro-samp-rows = مصادر الفهرس المميّزة: { $count }.

astro-survey-off = مسح السماء متوقف.
astro-survey-on = مسح السماء: { $name }. { $credit }
astro-survey-failed = تعذّر عرض { $name }: { $reason }
astro-survey-credit = { $name }: { $credit }

net-what-sky-survey = بلاطات مسح السماء من { $host }
