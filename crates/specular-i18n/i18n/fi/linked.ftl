# Linked views: several DICOM series side by side, scrolling together by position, with
# reference lines, a 3D cursor, window and zoom sync, and manual alignment of priors.

# machine-drafted, needs native review
palette-compare-series = Vertaile sarjoja (linkitetyt näkymät)
# machine-drafted, needs native review
palette-compare-layout = Seuraava näkymäasettelu
# machine-drafted, needs native review
palette-next-viewport = Seuraava näkymä
# machine-drafted, needs native review
palette-prev-viewport = Edellinen näkymä
# machine-drafted, needs native review
palette-reference-lines = Näytä tai piilota viitelinjat
# machine-drafted, needs native review
palette-sync-window = Seuraava ikkunasynkronointi
# machine-drafted, needs native review
palette-sync-zoom = Zoomaa ja panoroi yhdessä koon mukaan
# machine-drafted, needs native review
palette-link-series = Seuraava sarjalinkitys
# machine-drafted, needs native review
palette-align-series = Kohdista muut sarjat tähän
# machine-drafted, needs native review
palette-compare-series-help = Näytä useita sarjoja vierekkäin, ja selaa niitä yhdessä.
# machine-drafted, needs native review
palette-compare-layout-help = Näytä enemmän tai vähemmän näkymiä.
# machine-drafted, needs native review
palette-next-viewport-help = Tee seuraavasta näkymästä aktiivinen.
# machine-drafted, needs native review
palette-prev-viewport-help = Tee edellisestä näkymästä aktiivinen.
# machine-drafted, needs native review
palette-reference-lines-help = Piirrä, mistä aktiivinen leike leikkaa muut näkymät.
# machine-drafted, needs native review
palette-sync-window-help = Valitse, mitkä näkymät jakavat ikkuna/taso-muutoksen: pois, sama modaliteetti tai kaikki.
# machine-drafted, needs native review
palette-sync-zoom-help = Näytä kaikki näkymät samassa koossa ja panoroi niitä yhdessä.
# machine-drafted, needs native review
palette-link-series-help = Valitse, miten tämä sarja seuraa muita.
# machine-drafted, needs native review
palette-align-series-help = Kohdista aiempi sarja tähän näytettyjen leikkeiden kohdalla.
# machine-drafted, needs native review
disabled-not-comparing = Vertaile ensin sarjoja

# The bar above the viewports. Toggles show their state by colour; the others name it.
# machine-drafted, needs native review
linked-layout = Asettelu: { $cols } × { $rows }
# machine-drafted, needs native review
linked-lines = Viitelinjat
# machine-drafted, needs native review
linked-zoom = Zoomin synkronointi
# machine-drafted, needs native review
linked-window-sync = Ikkunasynkronointi: { $mode ->
    [off] pois
    [modality] sama modaliteetti
   *[all] kaikki
}
# machine-drafted, needs native review
linked-link = Linkitys: { $mode ->
    [frame] viitekehys
    [position] potilaan asento
   *[off] pois
}
# machine-drafted, needs native review
linked-align = Kohdista tähän

# One viewport. $active is "yes" for the active one.
# machine-drafted, needs native review
linked-viewport = { $active ->
    [yes] Näkymä { $n } / { $count }, aktiivinen
   *[no] Näkymä { $n } / { $count }
}
# machine-drafted, needs native review
linked-empty = Vedä sarja tähän
# machine-drafted, needs native review
linked-series = { $number } · { $modality } · { $description }
# machine-drafted, needs native review
linked-slice = Leike { $n } / { $count } · { $position } mm
# machine-drafted, needs native review
linked-in-gap = Aukossa: lähin leike { $distance } mm päässä
# machine-drafted, needs native review
linked-outside = Tämän sarjan ulkopuolella ({ $distance } mm)
# machine-drafted, needs native review
linked-not-linked = Ei linkitetty
# machine-drafted, needs native review
linked-aligned = Kohdistettu käsin ({ $offset } mm)
# machine-drafted, needs native review
linked-cursor = 3D-kohdistin kohdassa { $x }, { $y }, { $z } mm
# machine-drafted, needs native review
linked-chip = { $label }, { $count ->
        [one] { $count } kuva
       *[other] { $count } kuvaa
    }

# Toasts and announcements.
# machine-drafted, needs native review
linked-opened = { $count ->
    [one] Vain yksi sarja: vedä toinen tutkimusselaimesta
   *[other] Vertaillaan { $count } sarjaa
}
# machine-drafted, needs native review
linked-no-geometry = Tällä sarjalla ei ole leikepaikkoja: sitä ei voi linkittää
# machine-drafted, needs native review
linked-active = Näkymä { $n }: { $series }
# machine-drafted, needs native review
linked-aligned-count = { $count ->
    [0] Ei kohdistettavaa: muut sarjat jakavat tämän viitekehyksen
    [one] 1 sarja kohdistettu aktiiviseen, tämän istunnon ajaksi
   *[other] { $count } sarjaa kohdistettu aktiiviseen, tämän istunnon ajaksi
}
