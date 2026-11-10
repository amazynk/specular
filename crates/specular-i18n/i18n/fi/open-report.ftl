# machine-drafted, needs native review
# Specular — Avausraportti: miksi kuvan avaaminen kesti niin kauan (tietopaneeli, osio
# ”Suorituskyky”; DESIGN.md §12.7).
# Finnish case endings around placeables are kept neutral; native review should check them.

open-report-title = Suorituskyky

## Headlines
open-report-fast = Avattu: { $ms } ms
open-report-display-cache = Näytetty näyttövälimuistista
open-report-slow-read = Hidas tallennustila: tiedoston lukeminen vei { $percent } ajasta ({ $speed } Mt/s)
# machine-drafted, needs native review
open-report-slow-read-no-speed = Hidas tallennustila: tiedoston lukeminen vei { $percent } ajasta (pieni tiedosto: enimmäkseen tallennustilan vastauksen odottamista)
open-report-archive-read = Arkiston purkaminen vei { $percent } ajasta
open-report-startup = Sovelluksen { -app-name } käynnistys vei { $percent } ajasta
open-report-checked = Purettu kahdesti: laitteistopurkajan ensimmäistä tulosta verrattiin CPU:n tulokseen ({ $percent } ajasta)
open-report-hardware = Purettu laitteistopurkajalla, joka vei { $percent } ajasta
open-report-decode = Suuri kuva: { $mpx } megapikselin purkaminen vei { $percent } ajasta
open-report-upload = Kuvan lähettäminen GPU:lle vei { $percent } ajasta
open-report-waiting = Muun työn odottaminen vei { $percent } ajasta
open-report-stage = Vaihe { $stage } vei { $percent } ajasta
open-report-memory-short = Muistia oli vähän, joten järjestelmä on voinut joutua käyttämään sivutusta

## Why a decode ran on the processor although a hardware decoder can do it
open-report-sw-not-probed = Purettu ohjelmallisesti: laitteistopurkajaa tunnistettiin vielä
open-report-sw-no-device = Purettu ohjelmallisesti: { $api } ei ole käytettävissä tällä tietokoneella
open-report-sw-no-codec = Purettu ohjelmallisesti: tämän GPU:n { $api } ei sisällä purkajaa muodolle { $codec }
open-report-sw-blocked-mismatch = Purettu ohjelmallisesti: laitteistopurkaja poistettiin käytöstä, koska sen tulos ei vastannut CPU:n tulosta
open-report-sw-blocked-error = Purettu ohjelmallisesti: laitteistopurkaja poistettiin käytöstä virheen jälkeen
open-report-sw-blocked-lost = Purettu ohjelmallisesti: laitteistopurkaja poistettiin käytöstä, koska se lakkasi vastaamasta
open-report-sw-failed = Purettu ohjelmallisesti: laitteistopurkaja epäonnistui tämän kuvan kanssa
open-report-sw-busy = Purettu ohjelmallisesti: laitteistopurkaja oli varattu
open-report-sw-unsupported = Purettu ohjelmallisesti: laitteistopurkaja ei pysty käsittelemään tätä kuvaa
open-report-sw-unavailable = Purettu ohjelmallisesti: laitteistopurku on pois päältä, kunnes { -app-name } käynnistetään uudelleen
open-report-sw-cpu-only = Purettu ohjelmallisesti: käsittelyksi on asetettu ”Vain CPU”
open-report-sw-setting-off = Purettu ohjelmallisesti: laitteistopurku on poistettu käytöstä asetuksissa
open-report-sw-unchecked = Purettu ohjelmallisesti: laitteistopurkajaa ei ole vielä tarkistettu (sitä kokeillaan ensin taustalla valmisteltavaan kuvaan)
open-report-sw-lower-power = Purettu ohjelmallisesti: akkukäytössä CPU kuluttaa tähän vähemmän virtaa
open-report-sw-faster = Purettu ohjelmallisesti: nopeampi tällä tietokoneella (noin { $cpu } ms, laitteistopurkajalla { $hw } ms)
open-report-sw-scaled = Purettu ohjelmallisesti: laitteistopurkaja purkaa kuvia vain täysikokoisina

## Hints
open-report-hint-copy-local = Kopioi tiedosto paikalliselle levylle, niin se avautuu nopeammin.
open-report-hint-extract = Pura arkisto kansioon, niin sen sivut avautuvat nopeammin.
open-report-hint-try-again = Laitteistopurkajan kokeileminen uudelleen: Asetukset → Suorituskyky → Yritä uudelleen.
open-report-hint-automatic = Aseta ”Missä kuvat käsitellään” arvoon Automaattinen kohdassa Asetukset → Suorituskyky.
open-report-hint-turn-on = Aseta ”Laitteistopurku” arvoon Automaattinen kohdassa Asetukset → Suorituskyky.
open-report-hint-free-memory = Sulje muita ohjelmia vapauttaaksesi muistia.
open-report-hint-once-only = Vain ensimmäinen kuva tarkistetaan näin.

## Rows
open-report-total = Tarkkaan kuvaan
open-report-first-pixels = Ensimmäiset pikselit
# machine-drafted, needs native review
open-report-whole = Koko kuva näkyvissä
open-report-stage-startup = Käynnistys
open-report-stage-read = Tiedoston luku
open-report-stage-parse = Otsakkeen luku
open-report-stage-colour = Värinhallinnan valmistelu
open-report-stage-decode = Purku
open-report-stage-upload = Lähetys GPU:lle
open-report-stage-waiting = Muun työn odotus
open-report-decoder = Purkaja
open-report-decoder-checked = { $engine }, tarkistettu CPU:lla
open-report-decoder-cache = Näyttövälimuisti
# $scale is the reduction: 4 for a level a quarter of the full width and height.
open-report-decoder-scaled = { $engine }, koko 1/{ $scale }
open-report-decoder-scaled-full = { $engine }, täysikokoisena, sitten pienennetty kokoon 1/{ $scale }
open-report-decoder-scaled-no-index = { $engine }, koko 1/{ $scale }, ilman pikakäynnistyksen hakemistoa
open-report-colour-conversion = Värimuunnos

## Advisor: a pattern across recent opens, what would help and by how much (rows)
# $count and $total are counts of opens, $format a file format ("HEIC"), $speed a formatted
# number (MB/s), $low and $high formatted numbers ("2.5").
open-report-advice-recurring = Toistuva
open-report-advice-change = Auttaisi
open-report-advice-gain = Arvioitu hyöty
open-report-advice-software = { $count }/{ $total } viime hitaasta { $format }-kuvasta purettiin ohjelmallisesti tästä syystä
open-report-advice-slow-storage = { $count }/{ $total } viime hitaasta kuvasta viivästyi tiedoston lukemisen takia (noin { $speed } Mt/s)
# machine-drafted, needs native review
open-report-advice-slow-storage-no-speed = { $count }/{ $total } viime hitaasta kuvasta viivästyi tiedoston lukemisen takia (pienet tiedostot: enimmäkseen tallennustilan vastauksen odottamista)
open-report-advice-archive = { $count }/{ $total } viime hitaasta kuvasta viivästyi arkiston purkamisen takia
open-report-advice-hardware-decoder = GPU, jossa on { $codec }-laitteistopurkaja
open-report-advice-gain-range = Noin { $low }–{ $high } kertaa nopeampi
open-report-advice-gain-one = Noin { $low } kertaa nopeampi

## Performance insights: the indicator in the corner and the panel it opens
# machine-drafted, needs native review
# $total is a formatted duration ("240 ms"), $format a file format ("HEIC").
open-report-insights-title = Suorituskykyvihjeet
open-report-insights-indicator = Miksi tämä on hidas?
open-report-insights-announce = Suorituskykyvihje: kuvat avautuvat toistuvasti hitaasti samasta syystä. Avaa ”Miksi tämä on hidas?” nähdäksesi, mikä auttaisi.
open-report-insights-hide = Piilota
open-report-insights-last = Viimeisin hidas kuva: { $total } tarkkaan kuvaan
open-report-insights-now = Mitä voit tehdä nyt
open-report-insights-hardware = Laitteisto, joka auttaisi
open-report-insights-none = Ei toistuvaa hidastumista
open-report-insights-none-detail = Vihjeet tulevat näkyviin, kun useat kuvat avautuvat hitaasti samasta syystä. Löytääkseen ne { -app-name } säilyttää viikon hitaat avaukset tällä tietokoneella; mitään ei lähetetä minnekään.
open-report-insights-software = { $format }-kuvat puretaan CPU:lla
open-report-insights-storage = Tiedostojen lukeminen hidastaa kuvia
open-report-insights-archive = Arkistojen purkaminen hidastaa kuvia
open-report-insights-nothing-now = Mikään sovelluksen { -app-name } asetus ei muuta tätä.
open-report-insights-display-cache = Ota näyttövälimuisti käyttöön: aiemmin avaamasi kuvat avautuvat silloin nopeasta kopiosta tällä tietokoneella.
open-report-insights-prefetch = Pidä useampia viereisiä kuvia valmiina, jotta seuraavat valmistellaan katsellessasi.
open-report-insights-hw-ssd = Nopea sisäinen asema (NVMe-SSD) näille tiedostoille
open-report-insights-action-turn-on = Ota käyttöön
open-report-insights-action-try-again = Yritä uudelleen
open-report-insights-action-automatic = Aseta automaattiseksi
open-report-insights-dismiss-storage = Älä näytä enää tallennustilasta
open-report-insights-dismiss-decoder = Älä näytä enää purkamisesta
open-report-insights-off = Poista vihjeet käytöstä
open-report-insights-on = Ota vihjeet käyttöön
open-report-insights-restore = Näytä piilotetut vihjeet uudelleen
open-report-insights-power = Tämä tietokone säästää virtaa (akkukäytössä tai virransäästötilassa), mikä hidastaa purkamista CPU:lla. Verkkovirtaan kytkettynä kuvat saattavat avautua nopeammin.

## Performance insights in the gallery: the same, about making thumbnails
# machine-drafted, needs native review
# $count and $total are counts of thumbnails, $total in -last-thumbnail a formatted duration
# ("240 ms"), $format a file format ("HEIC"), $speed a formatted number (MB/s).
open-report-insights-announce-thumbnails = Suorituskykyvihje: pikkukuvat ilmestyvät toistuvasti hitaasti samasta syystä. Avaa ”Miksi tämä on hidas?” nähdäksesi, mikä auttaisi.
open-report-insights-last-thumbnail = Viimeisin hidas pikkukuva: { $total } sen luomiseen
open-report-insights-thumbnails-software = { $format }-kuvien pikkukuvat puretaan CPU:lla
open-report-insights-thumbnails-storage = Tiedostojen lukeminen hidastaa pikkukuvia
open-report-insights-thumbnails-archive = Arkistojen purkaminen hidastaa pikkukuvia
open-report-insights-thumbnail-cache = Pikkukuvia ei voi säilyttää tällä tietokoneella (pikkukuvavälimuisti ei ole käytettävissä), joten ne luodaan uudelleen aina, kun kansio näytetään.
open-report-advice-thumbnails-software = { $count }/{ $total } viime hitaasta { $format }-pikkukuvasta purettiin ohjelmallisesti tästä syystä
open-report-advice-thumbnails-slow-storage = { $count }/{ $total } viime hitaasta pikkukuvasta viivästyi tiedoston lukemisen takia (noin { $speed } Mt/s)
# machine-drafted, needs native review
open-report-advice-thumbnails-slow-storage-no-speed = { $count }/{ $total } viime hitaasta pikkukuvasta viivästyi tiedoston lukemisen takia (pienet tiedostot: enimmäkseen tallennustilan vastauksen odottamista)
open-report-advice-thumbnails-archive = { $count }/{ $total } viime hitaasta pikkukuvasta viivästyi arkiston purkamisen takia
