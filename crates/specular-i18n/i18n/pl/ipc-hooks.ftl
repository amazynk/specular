# machine-drafted, needs native review
# Remote control and events (Settings → Integration), and hooks: programs settings.toml
# names to run when something happens in the viewer.

settings-integration-ipc = Zdalne sterowanie i zdarzenia
settings-integration-ipc-help = Inne programy na tym komputerze, uruchomione na Twoim koncie, mogą sterować { -app-name } i dowiadywać się, co się w nim dzieje, przez prywatne gniazdo (specular ipc). Nic nie nasłuchuje w sieci.

## The hooks panel under "Remote control and events".
hooks-panel-title = Hooki
hooks-panel-intro = Programy, które settings.toml uruchamia, gdy coś się dzieje. Każdy pyta raz przed pierwszym uruchomieniem.
hooks-panel-none = Brak hooków. Dodaj je jako [[hooks]] w settings.toml.
hooks-panel-keywords = hooki hooks zdarzenia automatyzacja skrypty programy
# $events: the event names, joined ("marked, exported").
hooks-panel-on = Gdy { $events }
hooks-panel-untrusted = Pyta przed pierwszym uruchomieniem
hooks-panel-allowed = Dozwolony
hooks-panel-remove = Usuń…
hooks-panel-unused = Nieużywany

## Asking before a hook's first run (and again when its program or arguments change).
# $name: the hook's name; $program: the program; $events: what sets it off (hook-event-*,
# joined with hook-or).
hook-consent-title = Uruchamiać „{ $name }” ({ $program }), gdy { $events }?
hook-consent-detail = Uruchamia ten program z nazwami zdjęć: { $args }
hook-consent-trust = Zezwalaj tylko na programy, którym ufasz. Zapyta ponownie, jeśli hook się zmieni.
hook-consent-allow = Zezwól
hook-consent-not-now = Nie teraz
hook-or = { " " }lub{ " " }
hook-event-opened = zdjęcie zostanie otwarte
hook-event-marked = zdjęcia zostaną oznaczone
hook-event-keywords = zmienią się słowa kluczowe
hook-event-exported = zdjęcia zostaną wyeksportowane
hook-event-imported = zdjęcia zostaną zaimportowane
hook-event-captured = aparat wyśle zdjęcie
hook-event-develop-saved = edycja w Wywoływaniu zostanie zapisana
hook-event-any = cokolwiek się stanie

## Removing a hook (Settings › Integration › Hooks › Remove…).
hook-remove-title = Usunąć hook „{ $name }”?
hook-remove-detail = Zostanie usunięty z settings.toml. Program i Twoje pliki zostają na dysku.
hook-remove-ok = Usuń
hook-removed = Usunięto hook „{ $name }”.
hook-settings-unwritable = Nie można zmienić settings.toml: { $reason }

## A hook that did not work (a quiet note; the log has the details).
hook-failed = Hook „{ $name }” nie zadziałał: { $reason }
hook-failed-exit = zakończył się kodem { $code }
hook-failed-timeout = zatrzymano go po { $seconds } s
hook-failed-start = nie mógł się uruchomić ({ $reason })
hook-dropped = Hook „{ $name }” jest zajęty: { $count ->
    [one] pominięto { $count } zdarzenie.
    [few] pominięto { $count } zdarzenia.
    [many] pominięto { $count } zdarzeń.
   *[other] pominięto { $count } zdarzenia.
}

## An export started by another program (specular ipc export): nobody was asked.
export-recipes-remote-exists = { $count ->
    [one] istnieje { $count } plik, który zostałby zastąpiony: nic nie wyeksportowano
    [few] istnieją { $count } pliki, które zostałyby zastąpione: nic nie wyeksportowano
    [many] istnieje { $count } plików, które zostałyby zastąpione: nic nie wyeksportowano
   *[other] istnieje { $count } pliku, który zostałby zastąpiony: nic nie wyeksportowano
}
export-recipes-remote-skipped = { $file } istnieje
