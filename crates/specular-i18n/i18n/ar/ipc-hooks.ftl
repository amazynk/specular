# machine-drafted, needs native review
# Remote control and events (Settings → Integration), and hooks: programs settings.toml
# names to run when something happens in the viewer.

settings-integration-ipc = التحكم عن بُعد والأحداث
settings-integration-ipc-help = يمكن للبرامج الأخرى على هذا الحاسوب، العاملة باسمك، أن تتحكم في { -app-name } وأن تعرف ما يحدث فيه عبر مقبس خاص (specular ipc). لا شيء يستمع على الشبكة.

## The hooks panel under "Remote control and events".
hooks-panel-title = الخطافات
hooks-panel-intro = برامج يشغّلها settings.toml عند حدوث شيء ما. يسأل كل منها مرة واحدة قبل تشغيله الأول.
hooks-panel-none = لا توجد خطافات. أضفها كـ [[hooks]] في settings.toml.
hooks-panel-keywords = خطافات hooks أحداث أتمتة سكربتات برامج
# $events: the event names, joined ("marked, exported").
hooks-panel-on = عندما { $events }
hooks-panel-untrusted = يسأل قبل تشغيله الأول
hooks-panel-allowed = مسموح
hooks-panel-remove = إزالة…
hooks-panel-unused = غير مستخدم

## Asking before a hook's first run (and again when its program or arguments change).
# $name: the hook's name; $program: the program; $events: what sets it off (hook-event-*,
# joined with hook-or).
hook-consent-title = تشغيل «{ $name }» ({ $program }) عندما { $events }؟
hook-consent-detail = يبدأ هذا البرنامج بأسماء الصور: { $args }
hook-consent-trust = اسمح فقط بالبرامج التي تثق بها. سيسأل مجددًا إذا تغيّر الخطاف.
hook-consent-allow = السماح
hook-consent-not-now = ليس الآن
hook-or = { " " }أو{ " " }
hook-event-opened = تُفتح صورة
hook-event-marked = تُوسم صور
hook-event-keywords = تتغير الكلمات المفتاحية
hook-event-exported = تُصدَّر صور
hook-event-imported = تُستورد صور
hook-event-captured = ترسل كاميرا صورة
hook-event-develop-saved = يُحفظ تعديل في التحميض
hook-event-any = يحدث أي شيء

## Removing a hook (Settings › Integration › Hooks › Remove…).
hook-remove-title = إزالة الخطاف «{ $name }»؟
hook-remove-detail = يُزال من settings.toml. يبقى البرنامج وملفاتك على القرص.
hook-remove-ok = إزالة
hook-removed = أُزيل الخطاف «{ $name }».
hook-settings-unwritable = تعذّر تغيير settings.toml: { $reason }

## A hook that did not work (a quiet note; the log has the details).
hook-failed = لم يعمل الخطاف «{ $name }»: { $reason }
hook-failed-exit = انتهى بالرمز { $code }
hook-failed-timeout = أُوقف بعد { $seconds } ثانية
hook-failed-start = تعذّر بدؤه ({ $reason })
hook-dropped = الخطاف «{ $name }» مشغول: { $count ->
    [zero] لم يُتخطَّ أي حدث.
    [one] تُخطّي حدث واحد.
    [two] تُخطّي حدثان.
    [few] تُخطّيت { $count } أحداث.
    [many] تُخطّي { $count } حدثًا.
   *[other] تُخطّي { $count } حدث.
}

## An export started by another program (specular ipc export): nobody was asked.
export-recipes-remote-exists = { $count ->
    [zero] لا توجد ملفات سيستبدلها: لم يُصدَّر شيء
    [one] يوجد ملف سيستبدله: لم يُصدَّر شيء
    [two] يوجد ملفان سيستبدلهما: لم يُصدَّر شيء
    [few] توجد { $count } ملفات سيستبدلها: لم يُصدَّر شيء
    [many] يوجد { $count } ملفًا سيستبدلها: لم يُصدَّر شيء
   *[other] يوجد { $count } ملف سيستبدلها: لم يُصدَّر شيء
}
export-recipes-remote-skipped = { $file } موجود
