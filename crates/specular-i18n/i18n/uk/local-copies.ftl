# machine-drafted, needs native review
# Local copies for offline editing (the Library).

settings-library-local-copies-limit = Ліміт локальних копій
settings-library-local-copies-limit-help = Скільки місця щонайбільше можуть займати локальні копії в теці даних Specular.

local-keep-menu = Зберігати локальну копію…
local-remove-menu = Вилучити локальну копію…
local-keep-location-menu = Зберігати локальну копію цього розташування…
local-remove-location-menu = Вилучити локальну копію цього розташування…
local-keep-album-menu = Зберігати локальну копію цього альбому…
local-remove-album-menu = Вилучити локальну копію цього альбому…
local-remove-held-elsewhere = Ці копії належать до збереженого розташування чи альбому. Вилучіть їх там.

local-keep-ask = Зберігати локальну копію: { $item }?
local-keep-detail = { $count ->
    [one] { $count } фото.
    [few] { $count } фото.
    [many] { $count } фото.
   *[other] { $count } фото.
}
    Оригінали: файли як є.
    Проксі: raw-файли у 2560 пікселів, досить для проявлення. Інші файли копіюються повністю.
    Копії зберігаються в теці даних Specular. Оригінали не змінюються.
local-keep-originals = Зберігати оригінали
local-keep-proxies = Зберігати проксі
local-keeping = Копіювання: { $item }…
local-kept = { $item }: { $count ->
    [one] { $count } локальна копія
    [few] { $count } локальні копії
    [many] { $count } локальних копій
   *[other] { $count } локальної копії
}, { $size }.
local-kept-full = { $count ->
    [one] { $count } фото не вмістилося в ліміт розміру.
    [few] { $count } фото не вмістилися в ліміт розміру.
    [many] { $count } фото не вмістилися в ліміт розміру.
   *[other] { $count } фото не вмістилися в ліміт розміру.
}
local-kept-unreachable = { $count ->
    [one] Не вдалося прочитати { $count } фото.
    [few] Не вдалося прочитати { $count } фото.
    [many] Не вдалося прочитати { $count } фото.
   *[other] Не вдалося прочитати { $count } фото.
}

local-remove-ask = Вилучити локальну копію: { $item }?
local-remove-detail = Копії в теці даних Specular буде видалено. Оригінали залишаються недоторканими. Зміни, що чекають, зберігаються.
local-remove-ok = Вилучити локальну копію
local-removed = Локальну копію вилучено: { $item }, звільнено { $size }.

local-banner-offline = { $location } офлайн. Ви працюєте з локальними копіями.
local-banner-offline-many = Розташувань офлайн: { $count }. Ви працюєте з локальними копіями.
local-banner-waiting = { $count ->
    [one] { $count } зміна чекає на оригінал.
    [few] { $count } зміни чекають на оригінали.
    [many] { $count } змін чекають на оригінали.
   *[other] { $count } зміни чекають на оригінали.
}
local-banner-conflicts = { $count ->
    [one] { $count } файл змінився, поки ви були офлайн.
    [few] { $count } файли змінилися, поки ви були офлайн.
    [many] { $count } файлів змінилися, поки ви були офлайн.
   *[other] { $count } файлу змінилося, поки ви були офлайн.
}
local-review = Переглянути…
local-sidebar-originals = локальна копія
local-sidebar-proxies = локальні проксі

local-synced = { $count ->
    [one] { $count } зміну записано в оригінал.
    [few] { $count } зміни записано в оригінали.
    [many] { $count } змін записано в оригінали.
   *[other] { $count } зміни записано в оригінали.
}
local-sync-failed = { $count ->
    [one] Не вдалося записати { $count } зміну. Вона чекає далі.
    [few] Не вдалося записати { $count } зміни. Вони чекають далі.
    [many] Не вдалося записати { $count } змін. Вони чекають далі.
   *[other] Не вдалося записати { $count } зміни. Вони чекають далі.
}

local-conflict-title = { $file } змінився, поки ви були офлайн. Яку версію зберегти: { $what }?
local-conflict-yours = Ваша: { $text }
local-conflict-theirs = На диску: { $text }
local-conflict-later = Скасувати — вирішити пізніше. Нічого не буде перезаписано.
local-keep-mine = Зберегти вашу
local-keep-theirs = Зберегти версію файлу
local-what-marks = оцінка
local-what-keywords = ключові слова
local-what-fields = поля IPTC
local-what-edit = обробка
local-stars = { $count ->
    [0] Без зірок
    [one] { $count } зірка
    [few] { $count } зірки
    [many] { $count } зірок
   *[other] { $count } зірки
}
local-label = { $label ->
    [1] Червона
    [2] Жовта
    [3] Зелена
   *[4] Синя
}
local-pick = Відібране
local-reject = Відхилене
local-no-keywords = без ключових слів
local-changed-elsewhere = змінено деінде
local-photo-changed = змінилося саме фото
local-edit-here = оброблено тут, { $when }
local-edit-elsewhere = оброблено деінде, { $when }
local-edit-none = без обробки

local-export-ask = { $count ->
    [one] { $count } фото офлайн. Експортувати його з проксі?
    [few] { $count } фото офлайн. Експортувати їх із проксі?
    [many] { $count } фото офлайн. Експортувати їх із проксі?
   *[other] { $count } фото офлайн. Експортувати їх із проксі?
}
local-export-detail = Проксі мають 2560 пікселів за довшою стороною. Оригінали дають повний розмір.
local-export-proxy = Експортувати з проксі
local-export-wait = Чекати на оригінали
local-export-waiting = Експорт почнеться, коли оригінали повернуться.

local-panel-title = Локальні копії
local-panel-used = Зайнято { $used } з { $limit }.
local-panel-originals = { $count ->
    [one] { $count } оригінал
    [few] { $count } оригінали
    [many] { $count } оригіналів
   *[other] { $count } оригіналу
}
local-panel-proxies = { $count ->
    [one] { $count } фото, raw як проксі
    [few] { $count } фото, raw як проксі
    [many] { $count } фото, raw як проксі
   *[other] { $count } фото, raw як проксі
}
local-panel-remove = Вилучити…
local-panel-footer = Зберігається в { $folder }. Оригінали ніколи не змінюються.
local-panel-keywords = офлайн локальні копії проксі місце
