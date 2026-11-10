# Specular — Export slideshow as video (DESIGN.md §6.5). Ukrainian. Machine-drafted, needs native review.

palette-export-slideshow = Експортувати слайд-шоу як відео…

video-export-title = Експортувати слайд-шоу як відео
video-export-resolution = Роздільність
# $width and $height in pixels.
video-export-resolution-window = Вікно ({ $width } × { $height })
video-export-fps = Частота кадрів
# $fps frames per second, a number.
video-export-fps-value = { $fps } кадр/с
video-export-quality = Якість
video-export-quality-high = Висока
video-export-quality-standard = Стандартна
video-export-quality-compact = Компактна
video-export-images = { $count ->
    [one] { $count } зображення, з порядком, переходом і підписами слайд-шоу
    [few] { $count } зображення, з порядком, переходом і підписами слайд-шоу
    [many] { $count } зображень, з порядком, переходом і підписами слайд-шоу
   *[other] { $count } зображення, з порядком, переходом і підписами слайд-шоу
}
# $length a duration ("2:40"), $size a file size ("38 MB").
video-export-summary = Близько { $length }, близько { $size }
video-export-export = Експортувати…
video-export-replace = Замінити
# $file a file name.
video-export-replace-question = { $file } уже існує. Замінити?
# $percent a formatted percentage ("42%").
video-export-progress = Експортування… { $percent }
video-export-cancelling = Скасування…
# $file a file name, $size a file size ("38 MB").
video-export-saved = Збережено { $file } ({ $size })
video-export-skipped = { $count ->
    [one] { $count } зображення не вдалося прочитати, його пропущено.
    [few] { $count } зображення не вдалося прочитати, їх пропущено.
    [many] { $count } зображень не вдалося прочитати, їх пропущено.
   *[other] { $count } зображення не вдалося прочитати, їх пропущено.
}
video-export-failed = Не вдалося експортувати відео: { $reason }
video-export-cancelled = Експорт скасовано
video-export-no-images = Не вдалося прочитати жодного зображення.
video-export-show-in-folder = Показати в теці

## Toolbar item and compatibility (machine-drafted, needs native review)
settings-viewing-toolbar-items-export-slideshow = Експортувати слайд-шоу як відео
video-export-compatibility = Сумісність
video-export-compatibility-current = Сучасні програвачі
video-export-compatibility-all = Усі програвачі
