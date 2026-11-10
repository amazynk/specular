# machine-drafted, needs native review
## Håndbogens panel, nedbrudsrapporter, diagnosepakken, opdateringstjek og Om. Dansk.
## Stil: kort, klart, uden fyld; én tanke pr. sætning.

## Palet

palette-help-manual = Hjælp
palette-help-manual-help = Åbn håndbogen på siden for det, der vises.
palette-save-diagnostics = Gem diagnosepakke…
palette-save-diagnostics-help = Gem nedbrudsrapporter, loggen og systemoplysninger i en zip-fil. Intet sendes.
# Et paletpunkt, der åbner et afsnit i håndbogen; { $title } er en side eller »Side › Afsnit«.
palette-help-topic = Hjælp: { $title }
palette-help-topic-help = Åbn dette afsnit i håndbogen.
# Under en kommando i paletten, der ikke er tilgængelig.
palette-disabled-help = Ikke tilgængelig nu. Tryk på Enter for at læse hvorfor.

## Hjælpepanelet

help-title = Hjælp
help-search-placeholder = Søg i håndbogen
help-contents = Indhold
help-back = Tilbage
help-forward = Frem
help-no-results = Intet fundet for »{ $query }«.
help-results = { $count ->
    [one] 1 afsnit fundet
   *[other] { $count } afsnit fundet
}
help-section-start = Kom i gang
help-section-modes = Visning og tilstande
help-section-tasks = Hvordan gør jeg…
help-section-reference = Reference
help-keys-unbound = Ikke angivet
# En kommando i håndbogen uden tast; den køres fra paletten.
help-key-unbound = { $name } (palet)
help-keys-command = Kommando
help-keys-keys = Taster
help-commands = Alle kommandoer findes i kommandopaletten ({ $key }). Skriv en del af navnet.
help-commands-no-key = Alle kommandoer findes i kommandopaletten. Skriv en del af navnet.
help-link = Gå til { $title }
help-open = Hjælp
# Panelets navn for skærmlæsere; { $page } er den viste side.
help-label = Hjælp: { $page }
help-disabled-why = Hvorfor?

## Nedbrudsrapport tilbudt ved næste start

crash-title = { -app-name } lukkede uventet
crash-body = En rapport blev gemt på denne computer. Intet blev sendt.
crash-view = Vis rapport
crash-hide = Skjul rapport
crash-copy = Kopiér
crash-copied = Rapport kopieret
crash-save-bundle = Gem diagnosepakke…
crash-dismiss = Luk

## Diagnosepakke

diag-title = Gem diagnosepakke
diag-intro = En zip-fil til at vedhæfte en fejlrapport. Den gemmes på denne computer. Intet sendes.
diag-contents = Indhold
diag-include-names = Medtag filnavne
diag-include-names-help = Fra: navne erstattes af file-1.jpg, dir-1 og så videre.
diag-privacy = Din hjemmemappe vises som ~ og dit brugernavn som <user>. Adgangskoder, serveradresser og billeder medtages aldrig.
diag-where = Rapportér et problem: { $issues } · { $email }
diag-save = Gem…
diag-cancel = Annuller
diag-saved = Diagnosepakke gemt: { $path }
diag-failed = Kunne ikke gemme diagnosepakken: { $reason }
diag-entry-summary = Version, system, GPU og driver
diag-entry-crash = Seneste nedbrudsrapport
diag-entry-crashes = Tidligere nedbrudsrapporter ({ $count })
diag-entry-log = Sidste { $count } loglinjer
diag-entry-settings = Indstillinger, uden adgangskoder og adresser
diag-entry-gpu = GPU- og skærmoplysninger
diag-entry-readme = Hvad pakken er
diag-size = { $size }
# README.txt i pakken.
diag-readme =
    Diagnosepakke fra { -app-name }.

    Den indeholder det, der hjælper med at finde et problem: version og systemoplysninger, nedbrudsrapporter, slutningen af loggen og indstillingerne.

    Din hjemmemappe vises som ~ og dit brugernavn som <user>. Medmindre du valgte at medtage dem, erstattes fil- og mappenavne af file-1, dir-1 og så videre. Adgangskoder og serveradresser fjernes.

    Intet blev sendt. Du bestemmer, om du vil dele filen.

## Opdateringer (Indstillinger)

settings-section-updates = Opdateringer
settings-updates-check = Søg efter opdateringer
settings-updates-check-help = Én gang om dagen spørges, om der findes en ny version. Forespørgslen indeholder kun versionsnummeret: ingen id, ingen cookie. Fra: { -app-name } forbinder aldrig for opdateringer.
settings-updates-channel = Kanal
settings-updates-channel-help = Stabil: kun udgivelser. Beta: også testversioner.
settings-updates-channel-stable = Stabil
settings-updates-channel-beta = Beta
settings-updates-check-now = Søg nu
settings-updates-check-now-help = Søg én gang nu.
update-available = Version { $version } er tilgængelig
update-notes = Udgivelsesnoter
update-download = Hent
update-none = { -app-name } er opdateret.
update-checking = Søger efter opdateringer…
update-failed = Kunne ikke søge efter opdateringer: { $reason }
update-last-checked = Sidst tjekket { $when }

## Om

about-build = Build { $commit } · { $profile } · { $target }
about-build-no-commit = Build { $profile } · { $target }
about-help = Hjælp
about-diagnostics = Gem diagnosepakke…
# machine-drafted, needs native review
about-renderer = Rendererinfo…
about-licences-count = { $count ->
    [one] 1 komponent fra tredjepart
   *[other] { $count } komponenter fra tredjepart
}
about-licence-group = { $licence } ({ $count })
about-contact = Fejl: { $issues } · Support: { $email }
