# Specular — Local AI pack (DESIGN.md "Local AI pack", §10). Polish. Machine-drafted, needs native review.

## Settings → Local AI
settings-section-ai = Lokalna AI
settings-ai-pack = Pakiet lokalnej AI
# machine-drafted, needs native review
settings-ai-pack-help = Opcjonalne funkcje działające tylko na tym komputerze. Nic, co znajdą, go nie opuszcza. Wybierz model dla każdej funkcji; nic nie jest pobierane, dopóki go nie wybierzesz.
# machine-drafted, needs native review
settings-ai-install-from-file = Zainstaluj z pliku…
settings-ai-install-from-file-help = Instaluje pliki modeli skopiowane na ten komputer, dla komputerów bez internetu. Każdy plik jest sprawdzany z listą dostarczaną z { -app-name }.
settings-ai-install-from-folder = Zainstaluj z folderu…
settings-ai-install-from-folder-help = To samo dla każdego pliku modelu w folderze.
settings-ai-search = Wyszukiwanie według opisu
settings-ai-search-help = Znajduj zdjęcia, opisując je, np. „pies na plaży”, i lepiej grupuj podobne ujęcia.
settings-ai-text = Tekst na obrazach
settings-ai-text-help = Odczytuje tekst na zdjęciach i skanach, aby wyszukiwanie text: je znajdowało.
settings-ai-culling = Pomoc w selekcji
# machine-drafted, needs native review
settings-ai-culling-help = Oznacza zamknięte oczy, rozmycie i złą ekspozycję, aby pomóc wybrać najlepsze ujęcie z serii.
settings-ai-people = Osoby
# machine-drafted, needs native review
settings-ai-people-help = Znajduje twarze i grupuje je według osób na potrzeby wyszukiwania people:. Grupowanie jest wyłączone, dopóki nie włączysz go dla biblioteki. Dane twarzy można usunąć dla jednej osoby lub dla wszystkich naraz.

# The pack's and a feature's state. $percent is formatted ("25%"); $done and $total are
# formatted numbers.
ai-state = { $state ->
        [none] W tej kompilacji nie ma dostępnych modeli.
        [installed] Zainstalowano
        [indexing] Zainstalowano · zaindeksowano { $done } z { $total } obrazów
        [downloading] Pobieranie… { $percent }
        [failed] Pobieranie nie powiodło się
       *[available] Nie zainstalowano
    }
ai-removing = Usuwanie…
# machine-drafted, needs native review
# Buttons of the Local AI page.
ai-button = { $action ->
        [download] Pobierz
        [cancel] Anuluj
        [remove] Usuń
        [forget] Zapomnij
        [faces] Usuń wszystkie dane twarzy
       *[pack] Usuń pakiet
    }
# The question before deleting.
ai-confirm = { $action ->
        [faces] Usunąć wszystkie dane twarzy? Wszystkie znalezione twarze i osoby zostaną usunięte. Twoje zdjęcia pozostaną.
       *[pack] Usunąć pakiet lokalnej AI, jego modele i wszystko, co znalazł? Twoje zdjęcia nie zostaną zmienione.
    }

## Gallery search
gallery-query-needs-ai = { $field }: wymaga pakietu lokalnej AI (Ustawienia › Lokalna AI)
# machine-drafted, needs native review
gallery-query-ai-not-yet = { $field }: pakiet lokalnej AI nie potrafi jeszcze tego wyszukiwać

## Faces: People view, face boxes and culling flags
# machine-drafted, needs native review
# "Show faces" on the picture.
faces-shown = Twarze widoczne
# machine-drafted, needs native review
faces-hidden = Twarze ukryte
# machine-drafted, needs native review
# Beside a face box whose eyes look closed.
faces-eyes-closed = Zamknięte oczy
# machine-drafted, needs native review
# Culling flags on the cull mode strip.
cull-flag-eyes-closed = Zamknięte oczy
# machine-drafted, needs native review
cull-flag-eyes-closed-some = Zamknięte oczy: { $closed } z { $faces } twarzy
# machine-drafted, needs native review
cull-flag-blurred = Rozmyte
# machine-drafted, needs native review
cull-flag-dark = Za ciemne
# machine-drafted, needs native review
cull-flag-bright = Za jasne
# machine-drafted, needs native review
cull-flag-clipped = Przycięte światła lub cienie
# machine-drafted, needs native review
# The People view.
people-title = Osoby
# machine-drafted, needs native review
people-open = Osoby
# machine-drafted, needs native review
people-done = Gotowe
# machine-drafted, needs native review
people-back = Wstecz
# machine-drafted, needs native review
people-show-photos = Pokaż zdjęcia
# machine-drafted, needs native review
people-name = Nazwij…
# machine-drafted, needs native review
people-name-prompt = Nazwij tę osobę
# machine-drafted, needs native review
people-faces-open = Twarze
# machine-drafted, needs native review
people-faces-of = Twarze: { $name }
# machine-drafted, needs native review
people-merge = Scal
# machine-drafted, needs native review
people-merged = Scalone osoby: { $count }
# machine-drafted, needs native review
people-merge-ask = Scalić { $from } z { $into }?
# machine-drafted, needs native review
people-merge-ask-many = Scalić osoby ({ $count }) z { $into }?
# machine-drafted, needs native review
people-merge-ask-detail = Ich twarze staną się twarzami { $into }. Zdjęcia na dysku się nie zmienią.
# machine-drafted, needs native review
people-not-this = To nie ta osoba
# machine-drafted, needs native review
people-face-removed = Twarz odłączono od tej osoby. Zdjęcie pozostaje.
# machine-drafted, needs native review
# Someone not named yet.
people-unnamed = Osoba #{ $id }
# machine-drafted, needs native review
people-face-count = { $count ->
        [one] { $count } twarz
        [few] { $count } twarze
        [many] { $count } twarzy
       *[other] { $count } twarzy
    }
# machine-drafted, needs native review
people-count = { $count ->
        [one] { $count } osoba
        [few] { $count } osoby
        [many] { $count } osób
       *[other] { $count } osoby
    }
# machine-drafted, needs native review
people-face-a11y = Twarz { $index }
# machine-drafted, needs native review
people-loading = Wczytywanie…
# machine-drafted, needs native review
people-not-installed = Funkcja Osoby nie jest zainstalowana. Pobierz ją w Ustawienia › Lokalna AI.
# machine-drafted, needs native review
people-grouping-off = Grupowanie twarzy jest wyłączone dla tej biblioteki. Włącz „Grupuj twarze w osoby” w Ustawienia › Lokalna AI.
# machine-drafted, needs native review
people-none = Nie znaleziono jeszcze żadnych osób.

# machine-drafted, needs native review
# A feature's model: Default (the model Specular ships as the default, or the nearest one
# this computer can run) or a model by name, which stays that model on any computer. The
# page shows it only for a feature with more than one model; the dropdown labels Default
# with the model it means (ai-size-recommended-option) and names each model as the model
# list does. The option names below are the palette's and the command line's: model names
# are proper names, kept as they are.
settings-ai-search-size = Model
settings-ai-search-size-help = SigLIP 2 B/16 najlepiej znajduje zdjęcia, w wielu językach. B/32 indeksuje około cztery razy szybciej, dla wolniejszych kart graficznych.
settings-ai-search-size-recommended = Domyślny
# machine-drafted, needs native review
settings-ai-search-size-small = SigLIP 2 B/32
settings-ai-search-size-standard = PE Core S16
settings-ai-search-size-large = SigLIP 2 B/16
settings-ai-text-size = Model
settings-ai-text-size-help = PP-OCRv6 small czyta dokładniej; tiny jest szybszy.
settings-ai-text-size-recommended = Domyślny
settings-ai-text-size-small = PP-OCRv6 tiny
settings-ai-text-size-standard = PP-OCRv6 small
settings-ai-text-size-large = Duży
# machine-drafted, needs native review
settings-ai-culling-size = Model
settings-ai-culling-size-help = Model, który wykrywa zamknięte oczy.
settings-ai-culling-size-recommended = Domyślny
settings-ai-culling-size-small = Mały
# machine-drafted, needs native review
settings-ai-culling-size-standard = Face Landmarker
settings-ai-culling-size-large = Duży
# machine-drafted, needs native review
settings-ai-people-size = Model
settings-ai-people-size-help = SFace jest mały. AuraFace jest większy i wytrenowany na danych, które jego wydawca określa jako licencjonowane do użytku komercyjnego.
settings-ai-people-size-recommended = Domyślny
settings-ai-people-size-small = Mały
# machine-drafted, needs native review
settings-ai-people-size-standard = BlazeFace + SFace
settings-ai-people-size-large = BlazeFace + AuraFace
# A model size by name.
ai-tier = { $tier ->
        [small] Mały
        [large] Duży
       *[standard] Standardowy
    }
# machine-drafted, needs native review
# Default in the model dropdown, and the label of the "Back to default" row: with the model
# it means on this computer, "Default (SigLIP 2 B/16)". $size is the model's name.
ai-size-recommended-option = Domyślny ({ $size })
# Under the model setting: the default, or the nearest model this computer can run. $gpu is
# the graphics card's name.
ai-recommended-for = Domyślny dla karty { $gpu }: { $size }
ai-recommended = Domyślny dla tego komputera: { $size }
# Nothing fits: no size can run here.
ai-recommended-none = Żaden rozmiar nie mieści się w pamięci tego komputera.
# Under an installed feature: what its model runs on now. $gpu is the graphics card's name
# (used with "gpu" only). The CPU variants say why the graphics card is not used.
ai-runs-on = { $engine ->
        [gpu] Działa na: GPU ({ $gpu })
        [gpu-unnamed] Działa na: GPU
        [no-gpu] Działa na: CPU (nie znaleziono GPU)
        [differed] Działa na: CPU — wynik GPU różnił się od wyniku CPU
        [failed] Działa na: CPU — model zawiódł na GPU
        [setting] Działa na: CPU (przetwarzanie ustawiono na „Tylko CPU”)
        [checking] Działa na: CPU — trwa porównywanie GPU z nim
       *[cpu] Działa na: CPU
    }
# A size's row: its files on disk ($place "disk"), or what it would download, the memory it
# needs, and " · needs the GPU" only for a size that cannot run without one. $bytes and
# $memory are formatted sizes.
ai-size-detail = { $place ->
        [disk] { $bytes } na dysku
       *[download] { $bytes } do pobrania
    } · { $memory } pamięci{ $gpu ->
        [yes] { " · wymaga GPU" }
       *[no] { "" }
    }
# machine-drafted, needs native review
# Under a size that may not run on this computer (it is still shown, and choosing it asks
# first). "gpu": it needs a graphics card this computer does not have or cannot use;
# "memory": $need is the memory it needs, $have what this computer has for it (formatted
# sizes, "6 GB").
ai-size-unfit = { $why ->
        [gpu] Wymaga GPU, a ten komputer nie ma żadnego, którego mógłby użyć.
       *[memory] Wymaga { $need } pamięci, więcej niż { $have }, na które Specular pozwala AI na tym komputerze.
    }
# Over the size setting while it names a size other than the one installed and in use
# ($current, $chosen: ai-tier). The rows below have the buttons to change it.
ai-size-not-in-use = Używany jest rozmiar { $current }, nie { $chosen }.
# machine-drafted, needs native review
# Buttons of a size's row and of the size question: "use" chooses that size (one not
# downloaded; a downloaded one says Switch, ai-size-button), "back" returns the setting to
# Recommended, "anyway" keeps a size that may not run on this computer (ai-unfit-question).
# The screen reader adds the size's name to each.
ai-size-action = { $action ->
        [back] Wróć do domyślnego
        [anyway] Użyj mimo to
       *[use] Użyj tego rozmiaru
    }
# The size the setting chose is not installed while another one is ($current, ai-tier).
ai-size-chosen-missing = Wybrany, ale jeszcze nie pobrany: nadal używany jest rozmiar { $current }.
# What a size is now: from what is installed, never from the setting. "downloaded": its files
# are here but another size is in use; "missing": the chosen size, not downloaded.
ai-size-state = { $state ->
        [current] W użyciu
        [downloaded] Pobrany, nieużywany
        [missing] Nie pobrano
        [previous] W użyciu, dopóki nowy rozmiar nie zaindeksuje biblioteki
        [target] Pobieranie…
        [failed] Pobieranie nie powiodło się
       *[other] { "" }
    }
# A licence row's button (opens the licence's page).
ai-licence = Pokaż licencję
# Asked before a size change that downloads and re-indexes. $feature is the feature's name,
# $size the new size (ai-tier), $download a formatted size.
ai-switch-question = { $download ->
        [none] { $feature }: przełączyć na rozmiar { $size }? Biblioteka zostanie zaindeksowana ponownie; do tego czasu używany jest obecny rozmiar.
       *[other] { $feature }: przełączyć na rozmiar { $size }? Zostanie pobrane { $download }, a biblioteka zostanie zaindeksowana ponownie; do tego czasu używany jest obecny rozmiar.
    }
# machine-drafted, needs native review
# A size that may not run on this computer, chosen for a feature not installed yet: asked
# before the setting keeps it (ai-size-unfit says why, on the next line). $size is ai-tier,
# $feature the feature's name.
ai-unfit-question = { $feature }: mimo to użyć rozmiaru { $size }? Może nie działać na tym komputerze albo działać bardzo wolno.
# The recommendation moved: offered, never applied silently.
ai-suggest-question = { $feature }: rozmiar { $size } lepiej pasuje teraz do tego komputera. Przełączyć? { $download ->
        [none] Biblioteka zostanie zaindeksowana ponownie.
       *[other] Zostanie pobrane { $download }, a biblioteka zostanie zaindeksowana ponownie.
    }
ai-suggest-toast = Lokalna AI: inny rozmiar modelu lepiej pasuje do tego komputera (Ustawienia › Lokalna AI).
# A feature switching size: the new size ($size, ai-tier) indexes the library while the old one
# stays in use. $done and $total are formatted numbers.
ai-switching = Przełączanie na rozmiar { $size } · zaindeksowano { $done } z { $total } obrazów

## The model manager: what each model says besides its size and licence

# machine-drafted, needs native review
# A model or feature not installable yet.
ai-coming = Dostępne w późniejszej wersji
# machine-drafted, needs native review
# What a model reads. $languages: multilingual, english, or a script id (OCR packs).
ai-choice-languages = { $languages ->
        [multilingual] Wiele języków
        [english] Tylko angielski
        [cjk-latin] Chiński, japoński i alfabet łaciński
        [zh-latin] Chiński i alfabet łaciński
        [arabic] Arabski
        [hebrew] Hebrajski
        [korean] Koreański
        [cyrillic] Cyrylica
        [greek] Grecki
        [thai] Tajski
        [devanagari] Dewanagari
        [tamil] Tamilski
        [telugu] Telugu
        [eslav] Rosyjski, ukraiński i białoruski
        [latin] Alfabet łaciński (wiele języków europejskich)
       *[other] { $languages }
    }
# A hint about a model. $hint: faster, closed-eyes, small-faces.
ai-choice-hint = { $hint ->
        [faster] Szybszy, dla wolniejszych kart graficznych
        [closed-eyes] Wykrywa zamknięte oczy
        [small-faces] Lepszy przy małych twarzach
        [no-translate] Nie tłumaczy na angielski
       *[other] { "" }
    }
# A model's training data. $source: the data as its publisher names it ($named is "no" when
# it is not named). $terms: whether the publisher states the data's terms ("stated"), does
# not ("unstated"), or the data is licensed for research only ("research").
ai-choice-data = { $terms ->
        [stated] Dane treningowe: { $source }
        [research] Dane treningowe: { $source } (licencja tylko do badań)
       *[unstated] { $named ->
            [no] Dane treningowe: bez nazwy, warunki nie podane
           *[yes] Dane treningowe: { $source }, warunki nie podane
        }
    }
# Where a model downloads from. $host is a host name ("huggingface.co").
ai-choice-host = Pobierane z { $host }
# A model still coming, in a feature's row. $size is formatted, $licence an SPDX name.
ai-choice-coming = { $name }: { $size }, { $licence }
# People: grouping faces into people, per library.
ai-grouping = Grupuj twarze w osoby
ai-grouping-help = Wyłączone, dopóki nie włączysz go dla tej biblioteki. Wyłączenie zatrzymuje grupowanie; już zgrupowane twarze pozostają, dopóki ich nie usuniesz.
ai-grouping-state = { $on ->
        [yes] Włączone w tej bibliotece
       *[no] Wyłączone
    }
ai-grouping-button = { $on ->
        [yes] Wyłącz
       *[no] Włącz
    }
# machine-drafted, needs native review
# An add-on (a script's text recogniser) before its feature is downloaded. $feature is the
# feature's name ("Text in photos").
ai-addon-first = Najpierw pobierz { $feature }.
# machine-drafted, needs native review
# Asked before an add-on is removed. $name is the model's name ("PP-OCRv5 Arabic").
ai-addon-remove-title = Usunąć { $name }?
# machine-drafted, needs native review
ai-addon-remove-detail = Jego pliki modelu zostaną usunięte. Twoje zdjęcia i już znaleziony tekst pozostaną.
# machine-drafted, needs native review
# Text: reading the text in this library's photos, per library.
ai-text-indexing = Odczytuj tekst na zdjęciach w tej bibliotece
# machine-drafted, needs native review
ai-text-indexing-help = Wyłączone, dopóki nie włączysz go dla tej biblioteki. Potem text: w wyszukiwaniu Biblioteki znajduje zdjęcia po słowach, które na nich są.
# machine-drafted, needs native review
ai-text-indexing-state = { $on ->
        [yes] Włączone w tej bibliotece
       *[no] Wyłączone
    }

## Instalacja z pliku

# machine-drafted, needs native review
# After model files were copied in.
ai-sideload-ready = { $count ->
        [one] Skopiowano jeden plik modelu. Naciśnij Pobierz, aby go zainstalować.
        [few] Skopiowano { $count } pliki modeli. Naciśnij Pobierz, aby je zainstalować.
        [many] Skopiowano { $count } plików modeli. Naciśnij Pobierz, aby je zainstalować.
       *[other] Skopiowano { $count } pliku modelu. Naciśnij Pobierz, aby je zainstalować.
    }
ai-sideload-none = Nie znaleziono plików modeli.
# $file is a file name, or several separated by commas.
ai-sideload-unknown = { $file } to nie jest model znany { -app-name }. Nie został skopiowany.
ai-sideload-failed = Nie można skopiować { $file }: { $reason }
# Asked before using a file whose SHA-256 isn't in the shipped list. $model is the model it
# would be used as ("clip-vit-b32-1").
ai-unverified-title = Użyć niezweryfikowanego modelu?
ai-unverified-detail = { $file } nie ma na liście modeli dostarczanych z { -app-name }, więc nie można go sprawdzić. Zostanie użyty jako { $model }.
ai-unverified-use = Użyj niezweryfikowanego modelu

# machine-drafted, needs native review
ai-failed-internal = Coś poszło nie tak wewnątrz { -app-name }. Szczegóły są w dzienniku.
# The buttons under a model-size question: switch, or keep the size in use.
ai-size-button = { $action ->
        [switch] Przełącz
       *[keep] Zachowaj obecny rozmiar
    }
