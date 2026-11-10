# Specular — Profiles: what Specular is used for (the first-run question, Settings → Profile,
# the toolbar chip that turns a profile's tools on). العربية.

settings-section-profile = الملف الشخصي
settings-profile-capabilities = ما أستخدم { -app-name } من أجله
settings-profile-capabilities-help = يضيف كل اختيار أدواته: الأوضاع والأوامر وأزرار شريط الأدوات وصفحات الإعدادات. الاستخدام العام مفعَّل دائمًا، ويُفتح كل تنسيق أيًّا كان الاختيار.
settings-profile-capabilities-general = عام
settings-profile-capabilities-engineering = هندسة
settings-profile-capabilities-medical = طب
settings-profile-capabilities-science = علوم
# machine-drafted, needs native review
settings-profile-capabilities-maps = خرائط (GIS)
settings-profile-shown = عُرض مسبقًا
settings-profile-shown-help = سؤال التشغيل الأول وعروض «دائمًا» التي عُرضت من قبل، حتى لا تُعرض مرة أخرى.
# The first-run sheet's title; the choices are the settings-profile-capabilities-* labels.
profile-question = فيمَ ستستخدم { -app-name }؟
profile-done = متابعة
# Under the first-run sheet's title: what a choice does.
profile-intro = يضيف كل اختيار أدواته الخاصة: الأوضاع والأوامر وأزرار شريط الأدوات وصفحات الإعدادات. يُفتح كل ملف أيًّا كان اختيارك.
# Under each choice of the first-run sheet: what it is for.
# machine-drafted, needs native review
profile-about = { $capability ->
    [engineering] الرسومات الهندسية والنماذج ثلاثية الأبعاد.
    [medical] دراسات DICOM والأحجام الطبية، مع القياسات والدمج.
    [science] الفلك والفحص المجهري والبيانات العلمية.
    [maps] الخرائط ومسارات GPS ونماذج الارتفاع.
   *[general] الصور الفوتوغرافية والصور اليومية. مفعَّل دائمًا.
}
# Above Continue on the first-run sheet.
profile-later = يمكنك تغيير الاثنين في أي وقت: الملف الشخصي من الإعدادات › الملف الشخصي، والبداية من الإعدادات › عام.
# The toolbar chip shown while a medical (or science, engineering) image is on screen.
# machine-drafted, needs native review
profile-tools = { $capability ->
    [medical] أدوات طبية
    [science] أدوات علمية
    [maps] أدوات الخرائط
    [engineering] أدوات هندسية
   *[general] أدوات عامة
}
# Why a command of a profile that is off is greyed out in the palette.
# machine-drafted, needs native review
profile-tools-off = { $capability ->
    [medical] الأدوات الطبية متوقفة
    [science] الأدوات العلمية متوقفة
    [maps] أدوات الخرائط متوقفة
    [engineering] الأدوات الهندسية متوقفة
   *[general] الأدوات العامة متوقفة
}
# Toast after the chip turned the tools on; its button keeps them on.
# machine-drafted, needs native review
profile-tools-on = { $capability ->
    [medical] الأدوات الطبية مفعَّلة لهذه الجلسة
    [science] الأدوات العلمية مفعَّلة لهذه الجلسة
    [maps] أدوات الخرائط مفعَّلة لهذه الجلسة
    [engineering] الأدوات الهندسية مفعَّلة لهذه الجلسة
   *[general] الأدوات العامة مفعَّلة لهذه الجلسة
}
profile-always = دائمًا
# Palette entries; $tools is a profile-tools label.
profile-turn-on = { $always ->
    [yes] تفعيل { $tools } دائمًا
   *[no] تفعيل { $tools } لهذه الجلسة
}
