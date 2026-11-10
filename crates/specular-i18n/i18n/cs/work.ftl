# machine-drafted, needs native review
# Specular — Práce na pozadí: ukazatel, který se zobrazí, když něco běží na pozadí, a panel, který otevírá. Čeština.

## Panel

work-title = Práce na pozadí
work-show = Zobrazit práci na pozadí
work-empty = Na pozadí nic neběží.

## Úlohy

work-kind-library = Knihovna fotek
work-kind-ai-search = Hledání pomocí AI
work-kind-faces = Lidé
work-kind-text = Text ve fotkách
work-kind-culling = Hodnocení pro třídění
work-kind-download = Stahování modelu AI
work-kind-similar = Podobné snímky

## Kroky

work-step-finding = Hledání fotek
work-step-reading = Čtení údajů o fotkách
work-step-signatures = Porovnávání fotek
work-step-grouping = Seskupování podobných snímků
work-step-text-index = Vytváření indexu pro hledání
work-step-embedding = Indexování fotek pro hledání pomocí AI
work-step-switching = Nové indexování pro novou velikost modelu
work-step-faces = Hledání tváří
work-step-ocr = Čtení textu ve fotkách
work-step-scoring = Hodnocení ostrosti a expozice
work-step-downloading = Stahování
work-step-focus = Kontrola ostrosti
work-step-item = { $step } · { $item }

## Průběh

work-count-photos = { $done } z { $total } { $count ->
    [one] fotky
    [few] fotek
    [many] fotky
   *[other] fotek
}
work-count = { $done } z { $total }
work-count-left = { $count } · zbývá { $left }
work-so-far-photos = Zatím { $done } { $count ->
    [one] fotka
    [few] fotky
    [many] fotky
   *[other] fotek
}
work-so-far = Zatím { $done }

## Zbývající čas

work-eta-estimating = Odhaduje se…
work-eta-seconds = { $n ->
    [one] Zbývá asi { $n } sekunda
    [few] Zbývají asi { $n } sekundy
    [many] Zbývá asi { $n } sekundy
   *[other] Zbývá asi { $n } sekund
}
work-eta-minutes = { $n ->
    [one] Zbývá asi { $n } minuta
    [few] Zbývají asi { $n } minuty
    [many] Zbývá asi { $n } minuty
   *[other] Zbývá asi { $n } minut
}
work-eta-hours = { $h ->
    [one] Zbývá asi { $h } hodina
    [few] Zbývají asi { $h } hodiny
    [many] Zbývá asi { $h } hodiny
   *[other] Zbývá asi { $h } hodin
}
work-eta-hours-minutes = { $h ->
    [one] Zbývá asi { $h } hodina
    [few] Zbývají asi { $h } hodiny
    [many] Zbývá asi { $h } hodiny
   *[other] Zbývá asi { $h } hodin
} a { $m ->
    [one] { $m } minuta
    [few] { $m } minuty
    [many] { $m } minuty
   *[other] { $m } minut
}

## Proč úloha čeká

work-paused-browsing = Pozastaveno, dokud procházíte fotky
work-paused-battery = Pozastaveno při napájení z baterie
work-paused-low-power = Pozastaveno v úsporném režimu
work-paused-heat = Pozastaveno, dokud je počítač horký
work-paused-memory = Pozastaveno, dokud je málo paměti
work-paused-user = Pozastaveno

## Co následuje

work-then = Potom: { $steps }

## Tlačítka

work-pause = Pozastavit
work-resume = Pokračovat
work-settings = Nastavení…

## Ukazatel

work-chip = { $job } · { $done } z { $total }
work-chip-so-far = { $job } · { $done }
work-chip-paused = { $job } · { $reason }
work-chip-more = { $chip } (+{ $more })
work-chip-name = Práce na pozadí: { $status }

## Oznámení

work-started = { $job }: spuštěno na pozadí
work-finished = { $job }: dokončeno
