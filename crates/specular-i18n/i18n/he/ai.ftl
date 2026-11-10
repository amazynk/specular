# Specular — Local AI pack (DESIGN.md "Local AI pack", §10). Hebrew. Machine-drafted, needs native review.

## Settings → Local AI
settings-section-ai = בינה מלאכותית מקומית
settings-ai-pack = חבילת בינה מלאכותית מקומית
# machine-drafted, needs native review
settings-ai-pack-help = תכונות אופציונליות שפועלות רק במחשב הזה. שום דבר שהן מוצאות לא יוצא ממנו. בוחרים מודל לכל תכונה; שום דבר לא יורד עד שבוחרים בו.
# machine-drafted, needs native review
settings-ai-install-from-file = התקנה מקובץ…
settings-ai-install-from-file-help = התקנת קובצי מודל שהועתקו למחשב הזה, למחשבים ללא אינטרנט. כל קובץ נבדק מול הרשימה שמגיעה עם { -app-name }.
settings-ai-install-from-folder = התקנה מתיקייה…
settings-ai-install-from-folder-help = אותו הדבר, לכל קובץ מודל בתיקייה.
settings-ai-search = חיפוש לפי תיאור
settings-ai-search-help = מציאת תמונות לפי תיאור, כמו „כלב על החוף”, וקיבוץ טוב יותר של צילומים דומים.
settings-ai-text = טקסט בתמונות
settings-ai-text-help = קריאת הטקסט בתמונות ובסריקות, כדי שהחיפוש text: ימצא אותן.
settings-ai-culling = עזרה במיון
# machine-drafted, needs native review
settings-ai-culling-help = סימון עיניים עצומות, טשטוש וחשיפה גרועה כדי לעזור לבחור את הצילום הטוב ביותר מרצף.
settings-ai-people = אנשים
# machine-drafted, needs native review
settings-ai-people-help = מציאת פנים וקיבוצן לפי אדם, לחיפוש people:. הקיבוץ כבוי עד שמפעילים אותו בספרייה. אפשר למחוק נתוני פנים של אדם אחד או של כולם בבת אחת.

# The pack's and a feature's state. $percent is formatted ("25%"); $done and $total are
# formatted numbers.
ai-state = { $state ->
        [none] אין מודלים זמינים בגרסה הזו.
        [installed] מותקן
        [indexing] מותקן · { $done } מתוך { $total } תמונות עברו אינדוקס
        [downloading] מוריד… { $percent }
        [failed] ההורדה נכשלה
       *[available] לא מותקן
    }
ai-removing = מסיר…
# machine-drafted, needs native review
# Buttons of the Local AI page.
ai-button = { $action ->
        [download] הורדה
        [cancel] ביטול
        [remove] הסרה
        [forget] שכחה
        [faces] מחיקת כל נתוני הפנים
       *[pack] הסרת החבילה
    }
# The question before deleting.
ai-confirm = { $action ->
        [faces] למחוק את כל נתוני הפנים? כל הפנים והאנשים שנמצאו יימחקו. התמונות שלך נשארות.
       *[pack] להסיר את חבילת הבינה המלאכותית המקומית, את המודלים שלה ואת כל מה שמצאה? התמונות שלך לא ישתנו.
    }

## Gallery search
gallery-query-needs-ai = { $field }: נדרשת חבילת הבינה המלאכותית המקומית (הגדרות › בינה מלאכותית מקומית)
# machine-drafted, needs native review
gallery-query-ai-not-yet = { $field }: חבילת הבינה המלאכותית המקומית עדיין לא יכולה לחפש זאת

## Faces: People view, face boxes and culling flags
# machine-drafted, needs native review
# "Show faces" on the picture.
faces-shown = הפנים מוצגות
# machine-drafted, needs native review
faces-hidden = הפנים מוסתרות
# machine-drafted, needs native review
# Beside a face box whose eyes look closed.
faces-eyes-closed = עיניים עצומות
# machine-drafted, needs native review
# Culling flags on the cull mode strip.
cull-flag-eyes-closed = עיניים עצומות
# machine-drafted, needs native review
cull-flag-eyes-closed-some = עיניים עצומות: { $closed } מתוך { $faces } פנים
# machine-drafted, needs native review
cull-flag-blurred = מטושטש
# machine-drafted, needs native review
cull-flag-dark = כהה מדי
# machine-drafted, needs native review
cull-flag-bright = בהיר מדי
# machine-drafted, needs native review
cull-flag-clipped = אורות או צללים קטומים
# machine-drafted, needs native review
# The People view.
people-title = אנשים
# machine-drafted, needs native review
people-open = אנשים
# machine-drafted, needs native review
people-done = סיום
# machine-drafted, needs native review
people-back = חזרה
# machine-drafted, needs native review
people-show-photos = הצגת התמונות
# machine-drafted, needs native review
people-name = מתן שם…
# machine-drafted, needs native review
people-name-prompt = מתן שם לאדם הזה
# machine-drafted, needs native review
people-faces-open = פנים
# machine-drafted, needs native review
people-faces-of = הפנים של { $name }
# machine-drafted, needs native review
people-merge = מיזוג
# machine-drafted, needs native review
people-merged = { $count ->
        [one] אדם אחד מוזג
        [two] שני אנשים מוזגו
       *[other] { $count } אנשים מוזגו
    }
# machine-drafted, needs native review
people-merge-ask = למזג את { $from } לתוך { $into }?
# machine-drafted, needs native review
people-merge-ask-many = למזג { $count } אנשים לתוך { $into }?
# machine-drafted, needs native review
people-merge-ask-detail = הפנים שלהם יהפכו לפנים של { $into }. התמונות בדיסק לא משתנות.
# machine-drafted, needs native review
people-not-this = לא האדם הזה
# machine-drafted, needs native review
people-face-removed = הפנים הוסרו מהאדם הזה. התמונה נשארת.
# machine-drafted, needs native review
# Someone not named yet.
people-unnamed = אדם #{ $id }
# machine-drafted, needs native review
people-face-count = { $count ->
        [one] פנים אחד
        [two] { $count } פנים
       *[other] { $count } פנים
    }
# machine-drafted, needs native review
people-count = { $count ->
        [one] אדם אחד
        [two] שני אנשים
       *[other] { $count } אנשים
    }
# machine-drafted, needs native review
people-face-a11y = פנים { $index }
# machine-drafted, needs native review
people-loading = בטעינה…
# machine-drafted, needs native review
people-not-installed = התכונה אנשים לא מותקנת. אפשר להוריד אותה בהגדרות › בינה מלאכותית מקומית.
# machine-drafted, needs native review
people-grouping-off = קיבוץ הפנים כבוי בספרייה הזו. יש להפעיל את „קיבוץ פנים לאנשים” בהגדרות › בינה מלאכותית מקומית.
# machine-drafted, needs native review
people-none = עדיין לא נמצאו אנשים.

# machine-drafted, needs native review
# A feature's model: Default (the model Specular ships as the default, or the nearest one
# this computer can run) or a model by name, which stays that model on any computer. The
# page shows it only for a feature with more than one model; the dropdown labels Default
# with the model it means (ai-size-recommended-option) and names each model as the model
# list does. The option names below are the palette's and the command line's: model names
# are proper names, kept as they are.
settings-ai-search-size = מודל
settings-ai-search-size-help = המודל SigLIP 2 B/16 מוצא תמונות הכי טוב, בשפות רבות. המודל B/32 מבצע אינדוקס מהר פי ארבעה בערך, לכרטיסי מסך איטיים יותר.
settings-ai-search-size-recommended = ברירת מחדל
# machine-drafted, needs native review
settings-ai-search-size-small = SigLIP 2 B/32
settings-ai-search-size-standard = PE Core S16
settings-ai-search-size-large = SigLIP 2 B/16
settings-ai-text-size = מודל
settings-ai-text-size-help = המודל PP-OCRv6 small קורא בדיוק רב יותר; המודל tiny מהיר יותר.
settings-ai-text-size-recommended = ברירת מחדל
settings-ai-text-size-small = PP-OCRv6 tiny
settings-ai-text-size-standard = PP-OCRv6 small
settings-ai-text-size-large = גדול
# machine-drafted, needs native review
settings-ai-culling-size = מודל
settings-ai-culling-size-help = המודל שמזהה עיניים עצומות.
settings-ai-culling-size-recommended = ברירת מחדל
settings-ai-culling-size-small = קטן
# machine-drafted, needs native review
settings-ai-culling-size-standard = Face Landmarker
settings-ai-culling-size-large = גדול
# machine-drafted, needs native review
settings-ai-people-size = מודל
settings-ai-people-size-help = המודל SFace קטן. המודל AuraFace גדול יותר, ואומן על נתונים שלדברי המפרסם שלו מורשים לשימוש מסחרי.
settings-ai-people-size-recommended = ברירת מחדל
settings-ai-people-size-small = קטן
# machine-drafted, needs native review
settings-ai-people-size-standard = BlazeFace + SFace
settings-ai-people-size-large = BlazeFace + AuraFace
# A model size by name.
ai-tier = { $tier ->
        [small] קטן
        [large] גדול
       *[standard] רגיל
    }
# machine-drafted, needs native review
# Default in the model dropdown, and the label of the "Back to default" row: with the model
# it means on this computer, "Default (SigLIP 2 B/16)". $size is the model's name.
ai-size-recommended-option = ברירת מחדל ({ $size })
# Under the model setting: the default, or the nearest model this computer can run. $gpu is
# the graphics card's name.
ai-recommended-for = ברירת מחדל עבור { $gpu } שלך: { $size }
ai-recommended = ברירת מחדל למחשב הזה: { $size }
# Nothing fits: no size can run here.
ai-recommended-none = אף גודל לא נכנס בזיכרון של המחשב הזה.
# Under an installed feature: what its model runs on now. $gpu is the graphics card's name
# (used with "gpu" only). The CPU variants say why the graphics card is not used.
ai-runs-on = { $engine ->
        [gpu] פועל על: GPU ({ $gpu })
        [gpu-unnamed] פועל על: GPU
        [no-gpu] פועל על: CPU (לא נמצא GPU)
        [differed] פועל על: CPU — תוצאת ה-GPU שונה מזו של ה-CPU
        [failed] פועל על: CPU — המודל נכשל ב-GPU
        [setting] פועל על: CPU (העיבוד מוגדר ל„CPU בלבד”)
        [checking] פועל על: CPU — בודק את ה-GPU מולו
       *[cpu] פועל על: CPU
    }
# A size's row: its files on disk ($place "disk"), or what it would download, the memory it
# needs, and " · needs the GPU" only for a size that cannot run without one. $bytes and
# $memory are formatted sizes.
ai-size-detail = { $place ->
        [disk] { $bytes } בדיסק
       *[download] הורדה של { $bytes }
    } · זיכרון { $memory }{ $gpu ->
        [yes] { " · דורש GPU" }
       *[no] { "" }
    }
# machine-drafted, needs native review
# Under a size that may not run on this computer (it is still shown, and choosing it asks
# first). "gpu": it needs a graphics card this computer does not have or cannot use;
# "memory": $need is the memory it needs, $have what this computer has for it (formatted
# sizes, "6 GB").
ai-size-unfit = { $why ->
        [gpu] דורש GPU, ובמחשב הזה אין GPU שאפשר להשתמש בו.
       *[memory] דורש { $need } זיכרון, יותר מ־{ $have } ש־Specular מאפשר לבינה מלאכותית במחשב הזה.
    }
# Over the size setting while it names a size other than the one installed and in use
# ($current, $chosen: ai-tier). The rows below have the buttons to change it.
ai-size-not-in-use = { $current } בשימוש, לא { $chosen }.
# machine-drafted, needs native review
# Buttons of a size's row and of the size question: "use" chooses that size (one not
# downloaded; a downloaded one says Switch, ai-size-button), "back" returns the setting to
# Recommended, "anyway" keeps a size that may not run on this computer (ai-unfit-question).
# The screen reader adds the size's name to each.
ai-size-action = { $action ->
        [back] חזרה לברירת המחדל
        [anyway] להשתמש בכל זאת
       *[use] להשתמש בגודל הזה
    }
# The size the setting chose is not installed while another one is ($current, ai-tier).
ai-size-chosen-missing = נבחר, אבל עדיין לא הורד: { $current } נשאר בשימוש.
# What a size is now: from what is installed, never from the setting. "downloaded": its files
# are here but another size is in use; "missing": the chosen size, not downloaded.
ai-size-state = { $state ->
        [current] בשימוש
        [downloaded] הורד, לא בשימוש
        [missing] לא הורד
        [previous] בשימוש עד שהגודל החדש יסיים לבצע אינדוקס לספרייה שלך
        [target] מוריד…
        [failed] ההורדה נכשלה
       *[other] { "" }
    }
# A licence row's button (opens the licence's page).
ai-licence = הצגת הרישיון
# Asked before a size change that downloads and re-indexes. $feature is the feature's name,
# $size the new size (ai-tier), $download a formatted size.
ai-switch-question = { $download ->
        [none] { $feature }: לעבור לגודל { $size }? הספרייה שלך תעבור אינדוקס מחדש; עד אז הגודל הנוכחי נשאר בשימוש.
       *[other] { $feature }: לעבור לגודל { $size }? פעולה זו מורידה { $download } ומבצעת אינדוקס מחדש לספרייה שלך; עד אז הגודל הנוכחי נשאר בשימוש.
    }
# machine-drafted, needs native review
# A size that may not run on this computer, chosen for a feature not installed yet: asked
# before the setting keeps it (ai-size-unfit says why, on the next line). $size is ai-tier,
# $feature the feature's name.
ai-unfit-question = { $feature }: להשתמש בגודל { $size } בכל זאת? ייתכן שהוא לא יפעל במחשב הזה, או שיפעל רק לאט מאוד.
# The recommendation moved: offered, never applied silently.
ai-suggest-question = { $feature }: הגודל { $size } מתאים עכשיו יותר למחשב הזה. לעבור? { $download ->
        [none] הספרייה שלך תעבור אינדוקס מחדש.
       *[other] פעולה זו מורידה { $download } ומבצעת אינדוקס מחדש לספרייה שלך.
    }
ai-suggest-toast = בינה מלאכותית מקומית: גודל מודל אחר מתאים יותר למחשב הזה (הגדרות › בינה מלאכותית מקומית).
# A feature switching size: the new size ($size, ai-tier) indexes the library while the old one
# stays in use. $done and $total are formatted numbers.
ai-switching = עובר לגודל { $size } · { $done } מתוך { $total } תמונות עברו אינדוקס

## The model manager: what each model says besides its size and licence

# machine-drafted, needs native review
# A model or feature not installable yet.
ai-coming = יגיע בגרסה מאוחרת יותר
# machine-drafted, needs native review
# What a model reads. $languages: multilingual, english, or a script id (OCR packs).
ai-choice-languages = { $languages ->
        [multilingual] שפות רבות
        [english] אנגלית בלבד
        [cjk-latin] סינית, יפנית וכתב לטיני
        [zh-latin] סינית וכתב לטיני
        [arabic] ערבית
        [hebrew] עברית
        [korean] קוריאנית
        [cyrillic] קירילית
        [greek] יוונית
        [thai] תאית
        [devanagari] דוונאגרי
        [tamil] טמילית
        [telugu] טלוגו
        [eslav] רוסית, אוקראינית ובלארוסית
        [latin] כתב לטיני (שפות אירופיות רבות)
       *[other] { $languages }
    }
# A hint about a model. $hint: faster, closed-eyes, small-faces.
ai-choice-hint = { $hint ->
        [faster] מהיר יותר, לכרטיסי מסך איטיים יותר
        [closed-eyes] מזהה עיניים עצומות
        [small-faces] טוב יותר עם פנים קטנות
        [no-translate] לא מתרגם לאנגלית
       *[other] { "" }
    }
# A model's training data. $source: the data as its publisher names it ($named is "no" when
# it is not named). $terms: whether the publisher states the data's terms ("stated"), does
# not ("unstated"), or the data is licensed for research only ("research").
ai-choice-data = { $terms ->
        [stated] נתוני אימון: { $source }
        [research] נתוני אימון: { $source } (ברישיון למחקר בלבד)
       *[unstated] { $named ->
            [no] נתוני אימון: לא צוינו, התנאים לא צוינו
           *[yes] נתוני אימון: { $source }, התנאים לא צוינו
        }
    }
# Where a model downloads from. $host is a host name ("huggingface.co").
ai-choice-host = ההורדה מ־{ $host }
# A model still coming, in a feature's row. $size is formatted, $licence an SPDX name.
ai-choice-coming = { $name }: { $size }, { $licence }
# People: grouping faces into people, per library.
ai-grouping = קיבוץ פנים לאנשים
ai-grouping-help = כבוי עד שמפעילים אותו בספרייה הזו. כיבוי מפסיק את הקיבוץ; פנים שכבר קובצו נשארות עד שמוחקים אותן.
ai-grouping-state = { $on ->
        [yes] פועל בספרייה הזו
       *[no] כבוי
    }
ai-grouping-button = { $on ->
        [yes] כיבוי
       *[no] הפעלה
    }
# machine-drafted, needs native review
# An add-on (a script's text recogniser) before its feature is downloaded. $feature is the
# feature's name ("Text in photos").
ai-addon-first = צריך להוריד קודם את { $feature }.
# machine-drafted, needs native review
# Asked before an add-on is removed. $name is the model's name ("PP-OCRv5 Arabic").
ai-addon-remove-title = להסיר את { $name }?
# machine-drafted, needs native review
ai-addon-remove-detail = קובצי המודל שלו יימחקו. התמונות שלך והטקסט שכבר נמצא נשארים.
# machine-drafted, needs native review
# Text: reading the text in this library's photos, per library.
ai-text-indexing = קריאת הטקסט בתמונות של הספרייה הזו
# machine-drafted, needs native review
ai-text-indexing-help = כבוי עד שמפעילים אותו בספרייה הזו. אחר כך החיפוש text: בספרייה מוצא תמונות לפי המילים שבהן.
# machine-drafted, needs native review
ai-text-indexing-state = { $on ->
        [yes] פועל בספרייה הזו
       *[no] כבוי
    }

## התקנה מקובץ

# machine-drafted, needs native review
# After model files were copied in.
ai-sideload-ready = { $count ->
        [one] הועתק קובץ מודל אחד. יש ללחוץ על הורדה כדי להתקין אותו.
        [two] הועתקו שני קובצי מודל. יש ללחוץ על הורדה כדי להתקין אותם.
       *[other] הועתקו { $count } קובצי מודל. יש ללחוץ על הורדה כדי להתקין אותם.
    }
ai-sideload-none = לא נמצאו קובצי מודל.
# $file is a file name, or several separated by commas.
ai-sideload-unknown = { $file } אינו מודל ש־{ -app-name } מכיר. הוא לא הועתק.
ai-sideload-failed = לא ניתן להעתיק את { $file }: { $reason }
# Asked before using a file whose SHA-256 isn't in the shipped list. $model is the model it
# would be used as ("clip-vit-b32-1").
ai-unverified-title = להשתמש במודל שלא אומת?
ai-unverified-detail = { $file } אינו ברשימת המודלים שמגיעה עם { -app-name }, ולכן לא ניתן לבדוק אותו. הוא ישמש כ־{ $model }.
ai-unverified-use = שימוש במודל שלא אומת

# machine-drafted, needs native review
ai-failed-internal = משהו השתבש בתוך { -app-name }. הפרטים ביומן.
# The buttons under a model-size question: switch, or keep the size in use.
ai-size-button = { $action ->
        [switch] מעבר
       *[keep] שמירת הגודל הנוכחי
    }
