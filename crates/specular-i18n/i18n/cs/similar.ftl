# machine-drafted, needs native review
# Specular — duplikáty a podobné snímky (kategorie galerie). Čeština.

similar-duplicates = Duplikáty
similar-similar = Podobné snímky

## Položky nabídky Zobrazit a palety
similar-menu-entry = { $name } — { $groups ->
    [one] { $groups } skupina
    [few] { $groups } skupiny
    [many] { $groups } skupiny
   *[other] { $groups } skupin
}, uvolní se { $size }
similar-menu-scanning = { $name } — prohledávání ({ PERCENT($percent) })
similar-menu-none = { $name } — nic nenalezeno

## Záhlaví
similar-summary = { $groups ->
    [one] { $groups } skupina ke kontrole
    [few] { $groups } skupiny ke kontrole
    [many] { $groups } skupiny ke kontrole
   *[other] { $groups } skupin ke kontrole
}, { $files ->
    [one] { $files } soubor
    [few] { $files } soubory
    [many] { $files } souboru
   *[other] { $files } souborů
} k vyřazení, uvolní se { $size }
similar-progress = Vyřešeno { $resolved } z { $total }
similar-skipped = Přeskočeno: { $count }
similar-scanning = Hledání duplikátů a podobných snímků… { $done } z { $total }
# machine-drafted, needs native review
similar-checking-sharpness = Kontrola ostrosti… { $done } z { $total }
# machine-drafted, needs native review
# Seznam během prvního skenování (průběh je v záhlaví výše).
similar-waiting = Skupiny se zde zobrazí po dokončení skenování.
# machine-drafted, needs native review
similar-stopped = Skenování zastaveno: nic nebylo seskupeno
similar-stopped-sharpness = Kontrola ostrosti zastavena: nejlepší snímky vybrány podle náhledů
similar-stopped-empty = Skenování bylo zastaveno. Co už bylo změřeno, zůstává uloženo: Skenovat znovu naváže od tohoto místa.
similar-scan-again = Skenovat znovu
similar-empty-duplicates = V této složce nejsou žádné duplikáty.
similar-empty-similar = V této složce nejsou žádné podobné snímky.
similar-all-done = Všechny skupiny jsou zkontrolovány.

## Podobné snímky: jak moc si musí fotografie být podobné, aby se seskupily
similar-sensitivity = Citlivost
similar-sensitivity-strict = Přísná
similar-sensitivity-loose = Volná

## Zápatí seznamu skupin
similar-accept-all = Přijmout všechny návrhy
similar-show-skipped = Zobrazit přeskočené skupiny
similar-discard-with = Vyřadit: { $action }
similar-action-trash = Přesunout do koše
similar-action-move = Přesunout do složky…
similar-action-move-to = Přesunout do „{ $folder }“
similar-action-reject = Pouze označit jako zamítnuté
similar-choose-folder = Zvolit složku pro vyřazené soubory…

## Hromadné potvrzení
similar-confirm = Přijmout návrhy pro { $groups ->
    [one] { $groups } skupinu
    [few] { $groups } skupiny
    [many] { $groups } skupiny
   *[other] { $groups } skupin
}? { $files ->
    [one] { $files } soubor
    [few] { $files } soubory
    [many] { $files } souboru
   *[other] { $files } souborů
} ({ $size }): { $action }.
similar-confirm-yes = { $files ->
    [one] Přijmout a vyřadit { $files } soubor
    [few] Přijmout a vyřadit { $files } soubory
    [many] Přijmout a vyřadit { $files } souboru
   *[other] Přijmout a vyřadit { $files } souborů
}
similar-cancel = Zrušit

## Skupina
similar-group-title = Skupina { $index } z { $total }
similar-group-photos = { $count ->
    [one] { $count } fotografie
    [few] { $count } fotografie
    [many] { $count } fotografie
   *[other] { $count } fotografií
}
similar-to-free = Uvolní se { $size }

## Akce a stavy kontroly
similar-keep = Ponechat
similar-discard = Vyřadit
similar-undecided = Nerozhodnuto
similar-accept-next = Přijmout a další
similar-not-similar = Nejsou podobné
similar-skip = Přeskočit
similar-compare = Porovnat
similar-zoom = { PERCENT(100) }
similar-back = Zpět
similar-done = Hotovo
similar-suggested-keep = Návrh: ponechat
similar-suggested-discard = Návrh: vyřadit

## Oznámení
similar-toast-accepted = { $count ->
    [one] Přijata { $count } skupina
    [few] Přijaty { $count } skupiny
    [many] Přijato { $count } skupiny
   *[other] Přijato { $count } skupin
}
similar-toast-rejected = { $count ->
    [one] { $count } soubor označen jako zamítnutý
    [few] { $count } soubory označeny jako zamítnuté
    [many] { $count } souboru označeno jako zamítnuté
   *[other] { $count } souborů označeno jako zamítnuté
}
similar-toast-protected = Hodnocené, označené, oštítkované, otagované nebo upravené fotografie se vždy ponechají.
# machine-drafted, needs native review
similar-zoom-already = Tato fotka se už zobrazuje na { PERCENT(100) } nebo větší

## Usnadnění přístupu
similar-a11y-member = { $name }: { $state }, { $reason }

## Nápověda ke klávesám
similar-hint-list = ↑↓ výběr · Enter kontrola · A přijmout vše · Tab přepnout · Esc zavřít

## Proč se fotografie ponechá
similar-reason-rated = Hodnocená
similar-reason-flagged = Označená jako vybraná
similar-reason-labelled = Má barevný štítek
similar-reason-tagged = Má klíčová slova
similar-reason-edited = Má úpravy
similar-reason-original = Originál
similar-reason-raw-original = Originál RAW
similar-reason-highest-resolution = Nejvyšší rozlišení
similar-reason-deeper-colour = Větší barevná hloubka
similar-reason-least-compressed = Nejmenší komprese
similar-reason-has-metadata = Zachovává metadata
similar-reason-oldest = Nejstarší kopie
similar-reason-sharpest = Nejostřejší
similar-reason-steadiest = Nejméně rozmazaná pohybem
similar-reason-best-exposure = Nejlépe exponovaná
similar-reason-least-noise = Nejméně šumu
similar-reason-also-good = Také dobrá

## Proč se fotografie navrhuje k vyřazení
similar-reason-same-file = Shodný soubor
similar-reason-derivative = Odvozená kopie
similar-reason-rendering-of-raw = Zpracování souboru RAW
similar-reason-smaller-copy = Menší kopie
similar-reason-shallower-colour = Menší barevná hloubka
similar-reason-more-compressed = Větší komprese
similar-reason-no-metadata = Odstraněná metadata
similar-reason-newer-copy = Novější kopie
similar-reason-blurrier = Rozmazanější
similar-reason-motion-blur = Rozmazání pohybem
similar-reason-overexposed = Přeexponovaná
similar-reason-underexposed = Podexponovaná
similar-reason-noisier = Více šumu
similar-reason-lower-resolution = Nižší rozlišení
