# Specular — Settings → Keys, the key binding editor (DESIGN.md §11). machine-drafted, needs native review
# Key names inside messages ($keys) are never translated.

settings-keys-sequence-timeout = Odota seuraavaa näppäintä
settings-keys-sequence-timeout-help = Kuinka kauan näppäinsarja kuten G G odottaa seuraavaa näppäintään. Näppäin, joka aloittaa myös pidemmän sarjan, suoritetaan tämän tauon jälkeen.
settings-keys-bindings = Pikanäppäimet
settings-keys-bindings-help = Valitse toiminto ja paina näppäimiä, jotka haluat liittää siihen. Jokaisella tilalla on omat näppäimensä; karsintatila ja lääketieteelliset kuvat käyttävät muuten katselimen näppäimiä. Liitä Esc napauttamalla sitä; pitäminen pohjassa peruuttaa. Liitokset tallennetaan tiedostoon keymap.toml.
palette-customise-keys = Mukauta näppäimiä…
keys-mode-viewer = Katselin
keys-mode-cull = Karsintatila
keys-mode-medical = Lääketieteelliset kuvat
keys-modes = Tila
keys-presets = Esiasetus
keys-preset-default = Oletus
keys-preset-vim = Vim
keys-preset-culling = Lightroom-tyylinen karsinta
keys-preset-photo-mechanic = Photo Mechanic -tyylinen
action-pan-left = Siirrä vasemmalle
action-pan-right = Siirrä oikealle
action-pan-up = Siirrä ylös
action-pan-down = Siirrä alas
action-pan-page-left = Siirrä näytöllinen vasemmalle
action-pan-page-right = Siirrä näytöllinen oikealle
action-pan-page-up = Siirrä näytöllinen ylös
action-pan-page-down = Siirrä näytöllinen alas
action-reload = Lataa kuva uudelleen
keys-action-cull-pick = Valitse
keys-action-cull-reject = Hylkää
keys-action-cull-unflag = Poista merkintä
keys-action-cull-rate = { $stars ->
    [0] Poista arvio
    [one] Anna 1 tähti
   *[other] Anna { $stars } tähteä
}
keys-action-cull-label = Väritunniste: { $label }
keys-action-cull-copy-to = Kopioi kansioon { $slot }
keys-action-cull-move-to = Siirrä kansioon { $slot }
keys-import = Tuo…
keys-export = Vie…
keys-export-mode = Vie { $mode }…
keys-reset-mode = Palauta { $mode }
keys-reset-all = Palauta kaikki näppäimet
keys-add = Lisää näppäin toiminnolle { $action }
keys-change = Muuta { $keys }
keys-remove = Poista { $keys }
keys-reset-action = Palauta { $action }
keys-none = Ei näppäimiä
keys-changed = Muutettu
keys-press = Paina näppäimiä…
keys-recording-hint = Peruuta pitämällä Esc pohjassa
keys-cancel-recording = Peruuta
keys-no-match = Mikään toiminto tai näppäin ei vastaa hakua ”{ $query }”.
keys-list = Toiminnot ja niiden näppäimet
keys-confirm-preset = Käytetäänkö esiasetusta { $preset }? Se korvaa kaikki pikanäppäimesi.
keys-confirm-reset-mode = Palautetaanko kaikki tilan { $mode } näppäimet esiasetukseen?
keys-confirm-reset-all = Palautetaanko kaikkien tilojen kaikki näppäimet esiasetukseen?
keys-confirm-ok = Korvaa
keys-confirm-reset = Palauta
keys-confirm-cancel = Peruuta
keys-conflict-same = { $keys } suorittaa jo toiminnon ”{ $other }”.
keys-conflict-prefix = { $keys } ja { $sequence } (”{ $other }”) alkavat samoin: lyhyempi suoritetaan vasta tauon jälkeen.
keys-conflict-shadow = { $keys } suorittaa tilassa { $mode } toiminnon ”{ $other }”.
keys-conflict-reserved = { $keys } ei ehkä koskaan tavoita Specularia: { $reason }
keys-choice-replace = Korvaa
keys-choice-swap = Vaihda keskenään
keys-choice-keep = Pidä molemmat
keys-choice-anyway = Liitä silti
keys-choice-cancel = Peruuta
keys-reserved-switch = se vaihtaa ikkunaa.
keys-reserved-close = se sulkee ikkunan.
keys-reserved-secure = järjestelmä käyttää sitä.
keys-reserved-desktop = Super-pikanäppäimet kuuluvat työpöydälle.
keys-reserved-workspaces = se vaihtaa päätettä tai työtilaa.
keys-reserved-input = se vaihtaa syöttötapaa tai näppäimistöasettelua.
keys-reserved-macos = macOS käyttää sitä.
keys-announce-recording = Tallennetaan näppäimiä toiminnolle { $action }. Paina näppäimiä tai peruuta pitämällä Esc pohjassa.
keys-announce-key = { $keys }
keys-announce-bound = { $keys } suorittaa nyt toiminnon { $action }.
keys-announce-already = { $keys } suorittaa jo toiminnon { $action }.
keys-announce-cancelled = Tallennus peruttu.
keys-announce-removed = { $keys } poistettu toiminnosta { $action }.
keys-announce-reset-action = Toiminnolla { $action } on taas esiasetuksen näppäimet.
keys-announce-reset-mode = Tilan { $mode } näppäimet on palautettu.
keys-announce-reset-all = Kaikki näppäimet on palautettu.
keys-announce-preset = Käytössä on nyt esiasetus { $preset }.
keys-toast-imported = Pikanäppäimet tuotu
keys-toast-exported = Pikanäppäimet viety
keys-toast-reloaded = keymap.toml muuttui: pikanäppäimet ladattu uudelleen
keys-toast-error = Pikanäppäimiä ei voitu lukea: { $error }
keys-toast-write-error = keymap.toml-tiedostoa ei voitu tallentaa: { $error }
keys-pending = { $keys } …  { $next }
keys-problem-syntax = keymap.toml-tiedostoa ei voitu lukea: { $error }
keys-problem-key = Tuntematon näppäin ”{ $key }” ohitettiin.
keys-problem-action = ”{ $key }” nimeää tuntemattoman toiminnon ”{ $action }”; se ohitettiin.
keys-problem-mode = Taulukko [{ $mode }] ei ole tila; se ohitettiin.
keys-problem-duplicate = { $keys } on annettu kahdesti tilassa { $mode }; jälkimmäinen on voimassa.
keys-problem-prefix = { $keys } aloittaa sarjan { $longer } tilassa { $mode }, joten se suoritetaan vasta tauon jälkeen.
keys-problem-preset = Tuntematon esiasetus ”{ $preset }”; oletusnäppäimet ovat voimassa.

## keymap.toml editor (machine-drafted, needs native review)

keys-edit-file = Muokkaa keymap.toml-tiedostoa
keys-problem-action-suggestion = ”{ $key }” nimeää tuntemattoman toiminnon ”{ $action }”; se ohitettiin. Tarkoititko ”{ $suggestion }”?
keys-problem-mode-suggestion = Taulukko [{ $mode }] ei ole tila; se ohitettiin. Tarkoititko [{ $suggestion }]?
keys-problem-preset-suggestion = Tuntematon esiasetus ”{ $preset }”; oletusnäppäimet ovat voimassa. Tarkoititko ”{ $suggestion }”?
