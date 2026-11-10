# machine-drafted, needs native review
# Remote control and events (Settings → Integration), and hooks: programs settings.toml
# names to run when something happens in the viewer.

settings-integration-ipc = Дистанційне керування й події
settings-integration-ipc-help = Інші програми на цьому комп’ютері, запущені від вашого імені, можуть керувати { -app-name } і дізнаватися, що в ньому відбувається, через приватний сокет (specular ipc). У мережі ніщо не слухає.

## The hooks panel under "Remote control and events".
hooks-panel-title = Хуки
hooks-panel-intro = Програми, які settings.toml запускає, коли щось відбувається. Кожна питає один раз перед першим запуском.
hooks-panel-none = Хуків немає. Додайте їх як [[hooks]] у settings.toml.
hooks-panel-keywords = хуки hooks події автоматизація скрипти програми
# $events: the event names, joined ("marked, exported").
hooks-panel-on = Події: { $events }
hooks-panel-untrusted = Питає перед першим запуском
hooks-panel-allowed = Дозволено
hooks-panel-remove = Вилучити…
hooks-panel-unused = Не використовується

## Asking before a hook's first run (and again when its program or arguments change).
# $name: the hook's name; $program: the program; $events: what sets it off (hook-event-*,
# joined with hook-or).
hook-consent-title = Запускати «{ $name }» ({ $program }), коли { $events }?
hook-consent-detail = Він запускає цю програму з назвами фото: { $args }
hook-consent-trust = Дозволяйте лише програми, яким довіряєте. Якщо хук зміниться, запит з’явиться знову.
hook-consent-allow = Дозволити
hook-consent-not-now = Не зараз
hook-or = { " " }або{ " " }
hook-event-opened = відкрито фото
hook-event-marked = фото позначено
hook-event-keywords = змінено ключові слова
hook-event-exported = фото експортовано
hook-event-imported = фото імпортовано
hook-event-captured = камера надсилає фото
hook-event-develop-saved = збережено редагування в Проявленні
hook-event-any = будь-що відбувається

## Removing a hook (Settings › Integration › Hooks › Remove…).
hook-remove-title = Вилучити хук «{ $name }»?
hook-remove-detail = Його буде прибрано з settings.toml. Програма й ваші файли залишаться на диску.
hook-remove-ok = Вилучити
hook-removed = Хук «{ $name }» вилучено.
hook-settings-unwritable = Не вдалося змінити settings.toml: { $reason }

## A hook that did not work (a quiet note; the log has the details).
hook-failed = Хук «{ $name }» не спрацював: { $reason }
hook-failed-exit = він завершився з кодом { $code }
hook-failed-timeout = його зупинено через { $seconds } с
hook-failed-start = він не зміг запуститися ({ $reason })
hook-dropped = Хук «{ $name }» зайнятий: { $count ->
    [one] пропущено { $count } подію.
    [few] пропущено { $count } події.
    [many] пропущено { $count } подій.
   *[other] пропущено { $count } події.
}

## An export started by another program (specular ipc export): nobody was asked.
export-recipes-remote-exists = { $count ->
    [one] { $count } файл, який буде замінено, уже існує: нічого не експортовано
    [few] { $count } файли, які буде замінено, уже існують: нічого не експортовано
    [many] { $count } файлів, які буде замінено, уже існують: нічого не експортовано
   *[other] { $count } файлу, які буде замінено, уже існують: нічого не експортовано
}
export-recipes-remote-skipped = { $file } уже існує
