# The metadata editor: IPTC fields of the selected photos (gallery, Library, cull mode and
# the viewer).

palette-edit-metadata = Редагувати метадані
palette-edit-metadata-help = Редагувати назву, підпис, автора, права й місця вибраних фото.
metaedit-menu = Редагувати метадані…
metaedit-title = Метадані
metaedit-close = Закрити метадані
metaedit-photos = { $count ->
    [one] { $count } фото
    [few] { $count } фото
    [many] { $count } фото
   *[other] { $count } фото
}
metaedit-none-selected = Виберіть фото, щоб редагувати їхні метадані.
metaedit-reading = Читання метаданих…
metaedit-mixed = Різні: введіть, щоб задати для всіх
metaedit-clearing = Буде очищено на всіх фото. Esc залишає значення.
metaedit-keywords-others = у деяких є ще
metaedit-saved = Збережено: { $field }.
metaedit-cleared = Очищено: { $field }.
metaedit-write-failed = { $count ->
    [one] Метадані не збережено для { $count } фото: { $error }
    [few] Метадані не збережено для { $count } фото: { $error }
    [many] Метадані не збережено для { $count } фото: { $error }
   *[other] Метадані не збережено для { $count } фото: { $error }
}
metaedit-writes-sidecars = Зберігається в супровідних файлах XMP.
metaedit-writes-files = Зберігається у файлах JPEG, TIFF і DNG; для інших файлів — у супровідних файлах XMP.

metaedit-group-templates = Шаблони
metaedit-group-description = Опис
metaedit-group-creator = Автор
metaedit-group-rights = Права
metaedit-group-shown = Показане місце
metaedit-group-created = Місце створення
metaedit-group-workflow = Робочий процес
metaedit-templates-none = Шаблонів ще немає. Шаблон заповнює поля, якими ви часто користуєтеся.

metaedit-field-title = Назва
metaedit-field-headline = Заголовок
metaedit-field-description = Підпис
metaedit-field-keywords = Ключові слова (через ;)
metaedit-field-persons-shown = Особи на фото (через ;)
metaedit-field-event = Подія
metaedit-field-creator = Автор (через ;)
metaedit-field-creator-job-title = Посада автора
metaedit-field-contact-address = Адреса
metaedit-field-contact-city = Місто
metaedit-field-contact-region = Область чи провінція
metaedit-field-contact-postal-code = Поштовий індекс
metaedit-field-contact-country = Країна
metaedit-field-contact-phone = Телефон
metaedit-field-contact-email = Ел. пошта
metaedit-field-contact-url = Вебсайт
metaedit-field-copyright = Повідомлення про авторські права
metaedit-field-credit = Зазначення авторства
metaedit-field-source = Джерело
metaedit-field-usage-terms = Умови використання
metaedit-field-shown-sublocation = Район
metaedit-field-shown-city = Місто
metaedit-field-shown-state = Область чи провінція
metaedit-field-shown-country = Країна
metaedit-field-shown-country-code = Код країни
metaedit-field-created-sublocation = Район
metaedit-field-created-city = Місто
metaedit-field-created-state = Область чи провінція
metaedit-field-created-country = Країна
metaedit-field-created-country-code = Код країни
metaedit-field-instructions = Інструкції
metaedit-field-job-id = Ідентифікатор завдання

metaedit-template-new = Новий шаблон
metaedit-template-edit = Редагувати
metaedit-template-remove = Вилучити
metaedit-template-done = Готово
metaedit-template-name = Назва шаблону
metaedit-template-default-name = Шаблон { $n }
metaedit-template-title = Шаблон
metaedit-template-hint = Порожні поля не змінюються.
metaedit-template-replace-keywords = Замінити ключові слова фото
metaedit-template-applied = { $count ->
    [one] Шаблон «{ $name }» застосовано до { $count } фото.
    [few] Шаблон «{ $name }» застосовано до { $count } фото.
    [many] Шаблон «{ $name }» застосовано до { $count } фото.
   *[other] Шаблон «{ $name }» застосовано до { $count } фото.
}
metaedit-template-remove-ask = Вилучити шаблон «{ $name }»?
metaedit-template-remove-keep = Метадані фото залишаться.
metaedit-template-remove-yes = Вилучити
metaedit-template-removed = Шаблон «{ $name }» вилучено.

app-info-edit-metadata = Редагувати метадані…

settings-gallery-metadata-into-files = Записувати метадані у файли зображень
settings-gallery-metadata-into-files-help = Редактор метаданих також записує у файли JPEG, TIFF і DNG, нічого іншого в них не змінюючи. Вимкнено: лише супровідні файли XMP.
