# machine-drafted, needs native review
# Integraatio: avaa ohjelmalla, käyttäjän komennot, liittäminen (Stage 2).
palette-open-with = Avaa ohjelmalla…
palette-paste = Liitä
open-with-placeholder = Avaa ohjelmalla…
open-with-loading = Etsitään sovelluksia…
open-with-default = { $name } (oletus)
open-with-choose-other = Valitse toinen sovellus…
open-with-choose-title = Valitse sovellus
open-with-failed = Avaaminen ei onnistunut: { $name }: { $reason }
command-missing = Ohjelmaa ei löytynyt: { $program }
command-failed = Käynnistys epäonnistui: { $name }: { $reason }
command-exited = Päättyi virheeseen: { $name }: { $status }
paste-nothing = Leikepöydällä ei ole avattavaa
paste-failed = Kuvaa ei voitu liittää: { $reason }
drag-failed = Tiedostoa ei voitu vetää: { $reason }
about-storage = Asetukset: { $settings } · Välimuisti: { $cache }
about-storage-portable = Siirrettävä tila: asetukset ja välimuisti kansiossa { $dir }

## Scripting and recipes

palette-copy-view-command = Kopioi näkymä komentorivikomentona
palette-copy-view-python = Kopioi näkymä Pythonina
toast-copied-view-command = Komento kopioitu
script-copy-no-image = Ei kuvaa kuvattavaksi
recipe-confirm-title = Suoritetaanko resepti ”{ $name }” ensimmäistä kertaa? Se suorittaa: { $command }
recipe-confirm-run = Suorita
recipe-no-file = Resepti tarvitsee kuvatiedoston
recipe-running = { $name } käynnissä…
recipe-done = { $name }: { $shapes ->
    [one] 1 muoto
   *[other] { $shapes } muotoa
} lisätty
recipe-failed = { $name } epäonnistui: { $reason }
recipe-timeout = pysäytetty { $seconds } s jälkeen
recipe-no-output = { $name } ei kirjoittanut tiedostoa { $file }
