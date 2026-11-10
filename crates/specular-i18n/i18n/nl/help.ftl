# machine-drafted, needs native review
## Het handleidingpaneel, crashrapporten, het diagnosepakket, updatecontroles en Over. Nederlands.
## Stijl: kort, helder, zonder opvulling; één idee per zin.

## Palet

palette-help-manual = Help
palette-help-manual-help = De handleiding openen op de pagina over wat op het scherm staat.
palette-save-diagnostics = Diagnosepakket opslaan…
palette-save-diagnostics-help = Crashrapporten, het logboek en systeemgegevens opslaan in een zip-bestand. Er wordt niets verzonden.
# Paletitem dat een sectie van de handleiding opent; { $title } is een pagina of ‘Pagina › Sectie’.
palette-help-topic = Help: { $title }
palette-help-topic-help = Deze sectie van de handleiding openen.
# Onder een niet-beschikbare opdracht in het palet.
palette-disabled-help = Nu niet beschikbaar. Druk op Enter om te lezen waarom.

## Het helppaneel

help-title = Help
help-search-placeholder = Handleiding doorzoeken
help-contents = Inhoud
help-back = Terug
help-forward = Vooruit
help-no-results = Niets gevonden voor ‘{ $query }’.
help-results = { $count ->
    [one] 1 sectie gevonden
   *[other] { $count } secties gevonden
}
help-section-start = Aan de slag
help-section-modes = Weergave en modi
help-section-tasks = Hoe kan ik…
help-section-reference = Naslag
help-keys-unbound = Niet ingesteld
# Een opdracht in de handleiding zonder toets; die werkt via het palet.
help-key-unbound = { $name } (palet)
help-keys-command = Opdracht
help-keys-keys = Toetsen
help-commands = Elke opdracht staat in het opdrachtenpalet ({ $key }). Typ een deel van de naam.
help-commands-no-key = Elke opdracht staat in het opdrachtenpalet. Typ een deel van de naam.
help-link = Naar { $title }
help-open = Help
# Naam van het paneel voor schermlezers; { $page } is de getoonde pagina.
help-label = Help: { $page }
help-disabled-why = Waarom?

## Crashrapport aangeboden bij de volgende start

crash-title = { -app-name } is onverwacht afgesloten
crash-body = Er is een rapport op deze computer opgeslagen. Er is niets verzonden.
crash-view = Rapport bekijken
crash-hide = Rapport verbergen
crash-copy = Kopiëren
crash-copied = Rapport gekopieerd
crash-save-bundle = Diagnosepakket opslaan…
crash-dismiss = Sluiten

## Diagnosepakket

diag-title = Diagnosepakket opslaan
diag-intro = Een zip-bestand om bij een foutmelding te voegen. Het wordt op deze computer opgeslagen. Er wordt niets verzonden.
diag-contents = Inhoud
diag-include-names = Bestandsnamen opnemen
diag-include-names-help = Uit: namen worden vervangen door file-1.jpg, dir-1 enzovoort.
diag-privacy = Je persoonlijke map verschijnt als ~ en je gebruikersnaam als <user>. Wachtwoorden, serveradressen en afbeeldingen worden nooit opgenomen.
diag-where = Probleem melden: { $issues } · { $email }
diag-save = Opslaan…
diag-cancel = Annuleren
diag-saved = Diagnosepakket opgeslagen: { $path }
diag-failed = Kan het diagnosepakket niet opslaan: { $reason }
diag-entry-summary = Versie, systeem, GPU en stuurprogramma
diag-entry-crash = Laatste crashrapport
diag-entry-crashes = Eerdere crashrapporten ({ $count })
diag-entry-log = Laatste { $count } logregels
diag-entry-settings = Instellingen, zonder wachtwoorden of adressen
diag-entry-gpu = GPU- en beeldschermgegevens
diag-entry-readme = Wat dit pakket is
diag-size = { $size }
# De README.txt in het pakket.
diag-readme =
    Diagnosepakket van { -app-name }.

    Het bevat wat helpt een probleem te vinden: versie- en systeemgegevens, crashrapporten, het einde van het logboek en de instellingen.

    Je persoonlijke map verschijnt als ~ en je gebruikersnaam als <user>. Tenzij je ervoor koos ze op te nemen, worden bestands- en mapnamen vervangen door file-1, dir-1 enzovoort. Wachtwoorden en serveradressen worden verwijderd.

    Er is niets verzonden. Jij beslist of je dit bestand deelt.

## Updates (Instellingen)

settings-section-updates = Updates
settings-updates-check = Naar updates zoeken
settings-updates-check-help = Eén keer per dag vragen of er een nieuwe versie is. Het verzoek bevat alleen het versienummer: geen ID, geen cookie. Uit: { -app-name } maakt nooit verbinding voor updates.
settings-updates-channel = Kanaal
settings-updates-channel-help = Stabiel: alleen releases. Bèta: ook testversies.
settings-updates-channel-stable = Stabiel
settings-updates-channel-beta = Bèta
settings-updates-check-now = Nu controleren
settings-updates-check-now-help = Eén keer nu controleren.
update-available = Versie { $version } is beschikbaar
update-notes = Release-opmerkingen
update-download = Downloaden
update-none = { -app-name } is up-to-date.
update-checking = Zoeken naar updates…
update-failed = Kan niet naar updates zoeken: { $reason }
update-last-checked = Laatst gecontroleerd: { $when }

## Over

about-build = Build { $commit } · { $profile } · { $target }
about-build-no-commit = Build { $profile } · { $target }
about-help = Help
about-diagnostics = Diagnosepakket opslaan…
# machine-drafted, needs native review
about-renderer = Rendererinfo…
about-licences-count = { $count ->
    [one] 1 onderdeel van derden
   *[other] { $count } onderdelen van derden
}
about-licence-group = { $licence } ({ $count })
about-contact = Problemen: { $issues } · Ondersteuning: { $email }
