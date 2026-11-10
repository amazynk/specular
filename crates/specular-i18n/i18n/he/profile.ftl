# Specular — Profiles: what Specular is used for (the first-run question, Settings → Profile,
# the toolbar chip that turns a profile's tools on). עברית.

settings-section-profile = פרופיל
settings-profile-capabilities = למה אני משתמש ב־{ -app-name }
settings-profile-capabilities-help = כל בחירה מוסיפה את הכלים שלה: מצבים, פקודות, כפתורים בסרגל הכלים ודפים בהגדרות. כללי פעיל תמיד, וכל תבנית נפתחת בלי קשר לבחירה.
settings-profile-capabilities-general = כללי
settings-profile-capabilities-engineering = הנדסה
settings-profile-capabilities-medical = רפואה
settings-profile-capabilities-science = מדע
# machine-drafted, needs native review
settings-profile-capabilities-maps = מפות (GIS)
settings-profile-shown = כבר הוצע
settings-profile-shown-help = שאלת ההפעלה הראשונה והצעות „תמיד” שכבר הוצגו, כדי שלא יוצגו שוב.
# The first-run sheet's title; the choices are the settings-profile-capabilities-* labels.
profile-question = למה תשתמשו ב־{ -app-name }?
profile-done = המשך
# Under the first-run sheet's title: what a choice does.
profile-intro = כל בחירה מוסיפה כלים משלה: מצבים, פקודות, כפתורים בסרגל הכלים ודפים בהגדרות. כל קובץ נפתח בלי קשר לבחירה.
# Under each choice of the first-run sheet: what it is for.
# machine-drafted, needs native review
profile-about = { $capability ->
    [engineering] שרטוטים טכניים ומודלים בתלת-ממד.
    [medical] בדיקות DICOM ונפחים רפואיים, עם מדידות ומיזוג.
    [science] אסטרונומיה, מיקרוסקופיה ונתונים מדעיים.
    [maps] מפות, מסלולי GPS ומודלי גבהים.
   *[general] צילומים ותמונות יומיומיות. פעיל תמיד.
}
# Above Continue on the first-run sheet.
profile-later = אפשר לשנות את שניהם בכל עת: את הפרופיל דרך הגדרות › פרופיל, ואת מסך הפתיחה דרך הגדרות › כללי.
# The toolbar chip shown while a medical (or science, engineering) image is on screen.
# machine-drafted, needs native review
profile-tools = { $capability ->
    [medical] כלים רפואיים
    [science] כלים מדעיים
    [maps] כלי מפות
    [engineering] כלים הנדסיים
   *[general] כלים כלליים
}
# Why a command of a profile that is off is greyed out in the palette.
# machine-drafted, needs native review
profile-tools-off = { $capability ->
    [medical] הכלים הרפואיים כבויים
    [science] הכלים המדעיים כבויים
    [maps] כלי המפות כבויים
    [engineering] הכלים ההנדסיים כבויים
   *[general] הכלים הכלליים כבויים
}
# Toast after the chip turned the tools on; its button keeps them on.
# machine-drafted, needs native review
profile-tools-on = { $capability ->
    [medical] הכלים הרפואיים פעילים להפעלה זו
    [science] הכלים המדעיים פעילים להפעלה זו
    [maps] כלי המפות פעילים להפעלה זו
    [engineering] הכלים ההנדסיים פעילים להפעלה זו
   *[general] הכלים הכלליים פעילים להפעלה זו
}
profile-always = תמיד
# Palette entries; $tools is a profile-tools label.
profile-turn-on = { $always ->
    [yes] הפעלת { $tools } תמיד
   *[no] הפעלת { $tools } להפעלה זו
}
