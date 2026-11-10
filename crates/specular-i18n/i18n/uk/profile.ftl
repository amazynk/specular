# Specular — Profiles: what Specular is used for (the first-run question, Settings → Profile,
# the toolbar chip that turns a profile's tools on). Українська.

settings-section-profile = Профіль
settings-profile-capabilities = Для чого я використовую { -app-name }
settings-profile-capabilities-help = Кожен вибір додає свої інструменти: режими, команди, кнопки панелі інструментів і сторінки Налаштувань. Загальні завжди увімкнені, і кожен формат відкривається незалежно від вибору.
settings-profile-capabilities-general = Загальні
settings-profile-capabilities-engineering = Інженерія
settings-profile-capabilities-medical = Медицина
settings-profile-capabilities-science = Наука
# machine-drafted, needs native review
settings-profile-capabilities-maps = Мапи (ГІС)
settings-profile-shown = Уже запропоновано
settings-profile-shown-help = Питання під час першого запуску і вже показані пропозиції «Завжди», щоб вони не з’являлися знову.
# The first-run sheet's title; the choices are the settings-profile-capabilities-* labels.
profile-question = Для чого ви використовуватимете { -app-name }?
profile-done = Продовжити
# Under the first-run sheet's title: what a choice does.
profile-intro = Кожен вибір додає власні інструменти: режими, команди, кнопки панелі інструментів і сторінки Налаштувань. Кожен файл відкривається незалежно від вибору.
# Under each choice of the first-run sheet: what it is for.
# machine-drafted, needs native review ([maps] variant)
profile-about = { $capability ->
    [engineering] Технічні креслення і 3D-моделі.
    [medical] Дослідження DICOM і медичні об’єми, з вимірюваннями та суміщенням.
    [science] Астрономія, мікроскопія і наукові дані.
    [maps] Мапи, треки GPS і моделі висот.
   *[general] Фото і повсякденні зображення. Завжди увімкнено.
}
# Above Continue on the first-run sheet.
profile-later = Обидва можна будь-коли змінити: профіль у Налаштування › Профіль, початок у Налаштування › Загальні.
# The toolbar chip shown while a medical (or science, engineering) image is on screen.
# machine-drafted, needs native review ([maps] variant)
profile-tools = { $capability ->
    [medical] Медичні інструменти
    [science] Наукові інструменти
    [maps] Інструменти мап
    [engineering] Інженерні інструменти
   *[general] Загальні інструменти
}
# Why a command of a profile that is off is greyed out in the palette.
# machine-drafted, needs native review ([maps] variant)
profile-tools-off = { $capability ->
    [medical] Медичні інструменти вимкнено
    [science] Наукові інструменти вимкнено
    [maps] Інструменти мап вимкнено
    [engineering] Інженерні інструменти вимкнено
   *[general] Загальні інструменти вимкнено
}
# Toast after the chip turned the tools on; its button keeps them on.
# machine-drafted, needs native review ([maps] variant)
profile-tools-on = { $capability ->
    [medical] Медичні інструменти увімкнено на цей сеанс
    [science] Наукові інструменти увімкнено на цей сеанс
    [maps] Інструменти мап увімкнено на цей сеанс
    [engineering] Інженерні інструменти увімкнено на цей сеанс
   *[general] Загальні інструменти увімкнено на цей сеанс
}
profile-always = Завжди
# Palette entries; $tools is a profile-tools label.
profile-turn-on = { $always ->
    [yes] Завжди вмикати { $tools }
   *[no] Увімкнути { $tools } на цей сеанс
}
