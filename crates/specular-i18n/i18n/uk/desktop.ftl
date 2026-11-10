# machine-drafted, needs native review
# Глибока інтеграція з робочим столом: Налаштування → Інтеграція, встановлення шпалер.

settings-section-integration = Інтеграція
settings-integration-default-viewer = Типовий переглядач зображень
settings-integration-default-viewer-help = Відкривати зображення в { -app-name } подвійним клацанням. Вимкнення відновлює попередній переглядач.
# machine-drafted, needs native review
settings-integration-extra-default-types = Інші типи файлів
settings-integration-extra-default-types-help = Інші види файлів, які відкриваються в { -app-name } подвійним клацанням. Вибираються в пункті «Типовий переглядач зображень».
settings-integration-thumbnailer = Мініатюри у файловому менеджері
settings-integration-thumbnailer-help = Дозволити файловим менеджерам показувати мініатюри форматів, які вони не читають самі, як-от JPEG XL, HEIC і RAW-файли камер.
settings-integration-single-instance = Один екземпляр через D-Bus
settings-integration-single-instance-help = Файли, відкриті з файлового менеджера, панелі запуску чи термінала, потрапляють у вже запущене вікно { -app-name }. Діє для вікон, відкритих після зміни.
settings-integration-file-manager-actions = Дії у файловому менеджері
settings-integration-file-manager-actions-help = Додати «Відкрити», «Порівняти» та «Встановити як шпалери» до контекстних меню Nautilus, Dolphin, Thunar і Nemo.
settings-integration-search-provider = Пошук на робочому столі
# machine-drafted, needs native review
settings-integration-search-provider-help = Шукати зображення з бібліотеки в пошуку GNOME і KDE. Слова працюють так само, як у рядку пошуку бібліотеки.
# machine-drafted, needs native review
settings-integration-url-scheme = Відкривати посилання specular://
settings-integration-url-scheme-help = Посилання на кшталт specular:///home/me/photo.jpg у браузері чи іншій програмі відкриваються в { -app-name }. Відкриваються лише файли на цьому комп’ютері, а незвичні — тільки після вашого підтвердження.

integration-state-on = Увімкнено
integration-state-off = Вимкнено
integration-state-partial = Налаштовано частково. Застосуйте ще раз, щоб виправити.
integration-state-unavailable = Недоступно в цьому робочому середовищі
integration-state-sandboxed = Недоступно в пісочниці (Flatpak)
integration-state-no-bus = Недоступно: немає сеансу D-Bus
integration-state-no-file-manager = Недоступно: не знайдено підтримуваного файлового менеджера
integration-state-no-search = Недоступно: не знайдено ні GNOME Shell, ні KRunner
integration-state-working = Виконується…
integration-state-details = { $state } · { $details }
integration-detail-types = типи зображень: { $count } з { $total }
integration-detail-thumbnail-types = типи зображень: { $count }
integration-apply = Застосувати
integration-remove = Вилучити
integration-done-on = «{ $option }» увімкнено
integration-done-off = «{ $option }» вимкнено
integration-failed = Не вдалося змінити «{ $option }»: { $reason }
# machine-drafted, needs native review
integration-search-remove-ask = Вимкнути пошук на робочому столі?
integration-search-remove-detail = { -app-name } буде вилучено з пошуку { $desktops }. Бібліотека і зображення залишаться.
search-provider-open-folder = Відкрити теку

integration-label-open = Відкрити в { -app-name }
integration-label-compare = Порівняти в { -app-name }
integration-label-wallpaper = Встановити як шпалери
integration-label-generic-name = Переглядач зображень
integration-label-comment = Миттєво відкривайте й гортайте великі фотографії

palette-set-wallpaper = Встановити як шпалери
wallpaper-set = Шпалери встановлено ({ $method })
wallpaper-failed = Не вдалося встановити шпалери: { $reason }

# machine-drafted, needs native review
url-invalid = Не вдається відкрити це посилання { -app-name }: { $reason }
url-not-found = Файлів, указаних у посиланні, не існує
url-remote-refused = { -app-name } відкриває лише файли на цьому комп’ютері, а не { $source }
url-confirm-files = { $more ->
    [0] «{ $file }»
   *[other] «{ $file }» та інші (+{ $more })
}
url-confirm-title = { $reason ->
    [dots] Відкрити { url-confirm-files }? Посилання веде до нього через теки «..».
    [network] Відкрити { url-confirm-files }? Він на мережевому ресурсі або пристрої.
   *[hidden] Відкрити { url-confirm-files }? Назва містить невидимі символи або символи напрямку тексту.
}
url-confirm-open = Відкрити

# Launcher actions and recent files: machine-drafted, needs native review
integration-label-new-window = Нове вікно
integration-label-library = Бібліотека
integration-label-slideshow = Показ слайдів із Зображень
settings-integration-recent-files = Недавні файли стільниці
settings-integration-recent-files-help = Відкриті зображення з’являються в розділі «Недавні» файлового менеджера та в списках недавніх файлів стільниці. Вилучення прибирає записи { -app-name } із цих списків.
integration-detail-recent = файли: { $count }
# machine-drafted, needs native review
integration-extra-title = Інші типи файлів
# machine-drafted, needs native review
integration-extra-intro = Окрім зображень, { -app-name } може відкривати ці файли подвійним клацанням. Нічого не позначено, доки ви не позначите самі. Програми, що редагують ці файли, лишаються в меню «Відкрити за допомогою», а зняття позначки повертає тип програмі, яка була раніше. Для Blackmagic RAW, DWG, STEP, IGES і Zeiss CZI також потрібні безкоштовні програми від їхніх розробників: див. Налаштування › Додаткові декодери.
integration-extra-chosen = { $count ->
    [0] Нічого не вибрано
    [one] Вибрано { $count } тип
    [few] Вибрано { $count } типи
    [many] Вибрано { $count } типів
   *[other] Вибрано { $count } типу
}
integration-extra-count = { $on } з { $total }
integration-extra-none-yet = Поки немає
integration-extra-keywords = типи файлів формати асоціації подвійне клацання відкрити за допомогою редактор наука медицина CAD САПР GIS ГІС 3D молекули відео SVG PDF
integration-extra-group-editors = Файли редакторів
# machine-drafted, needs native review
integration-extra-group-editors-help = Документи програм для малювання й креслення: Krita, GIMP, Paint.NET, PaintShop Pro, Procreate, Affinity, OpenRaster, Illustrator, CorelDRAW, Clip Studio Paint, Sketch та Figma. Подвійне клацання показує зображення; редагуєте ви його й далі в програмі, де його створено.
integration-extra-group-science = Наука
integration-extra-group-science-help = Файли з приладів і дослідницьких програм: тепловізори, мікроскопи, вимірювання поверхонь, HDF5, NetCDF, сейсмічні й метеодані.
integration-extra-group-medical = Медицина
integration-extra-group-medical-help = Медичні знімки й нейровізуалізація: DICOM, NIfTI, MINC, NRRD, трактографія та поверхні мозку.
integration-extra-group-cad-gis = САПР і ГІС
# machine-drafted, needs native review
integration-extra-group-cad-gis-help = Креслення плотера, технічні ілюстрації, топології мікросхем і мапи: HP-GL, CGM, GDSII, shapefile, GeoPackage, KML, GPX, GeoJSON і тайли висот. DWG відкривається безкоштовним ODA File Converter, STEP і IGES — за допомогою OpenCASCADE.
integration-extra-group-models = 3D і молекули
integration-extra-group-models-help = 3D-моделі, моделі будівель, хмари точок і молекули: STL, OBJ, glTF, PLY, 3MF, X3D, IFC, LAS, E57, PDB, CIF тощо.
integration-extra-group-videos = Відео
# machine-drafted, needs native review
integration-extra-group-videos-help = Відео: MP4, MOV, M4V, 3GP, 3G2, MKV, WebM, AVCHD (MTS, M2TS), OGV, IVF і MXF. Blackmagic RAW відкривається безкоштовним Blackmagic RAW SDK.
integration-extra-group-other-images = SVG та інші зображення
# machine-drafted, needs native review
integration-extra-group-other-images-help = Зображення та документи, які зазвичай відкривають інші програми: креслення SVG, PDF, XPS, DjVu, EPS, WMF і EMF, Visio, OpenDocument, WordPerfect Graphics, книги EPUB і шрифти, а також анімації MNG, FLIC та анімовані курсори.
integration-extra-group-text = Текст, таблиці та субтитри
integration-extra-group-text-help = Звичайний текст і журнали, NFO, Markdown, таблиці CSV і TSV, RTF і файли субтитрів: SRT, WebVTT, ASS, SSA, MicroDVD, PGS, LRC, TTML і SCC.
