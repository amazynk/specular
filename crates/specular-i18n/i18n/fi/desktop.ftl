# machine-drafted, needs native review
# Syvä työpöytäintegraatio: Asetukset → Integraatio, aseta taustakuvaksi.

settings-section-integration = Integraatio
settings-integration-default-viewer = Oletuskuvankatselin
settings-integration-default-viewer-help = Avaa kuvat { -app-name }illa, kun kaksoisnapsautat niitä. Poistaminen palauttaa aiemman katseluohjelman.
# machine-drafted, needs native review
settings-integration-extra-default-types = Lisää tiedostotyyppejä
settings-integration-extra-default-types-help = Muut tiedostotyypit, jotka avautuvat kaksoisnapsautuksella sovelluksessa { -app-name }. Valitaan kohdassa Oletuskuvankatselin.
settings-integration-thumbnailer = Tiedostonhallinnan pikkukuvat
settings-integration-thumbnailer-help = Anna tiedostonhallintojen näyttää esikatselut muodoista, joita ne eivät itse osaa lukea, kuten JPEG XL, HEIC ja kameran RAW-tiedostot.
settings-integration-single-instance = Yksi ilmentymä D-Busin kautta
settings-integration-single-instance-help = Tiedostonhallinnasta, käynnistimestä tai päätteestä avatut tiedostot avautuvat käynnissä olevaan { -app-name }-ikkunaan. Koskee myöhemmin avattavia ikkunoita.
settings-integration-file-manager-actions = Tiedostonhallinnan toiminnot
settings-integration-file-manager-actions-help = Lisää ”Avaa”, ”Vertaa” ja ”Aseta taustakuvaksi” Nautiluksen, Dolphinin, Thunarin ja Nemon pikavalikkoihin.
settings-integration-search-provider = Työpöytähaku
# machine-drafted, needs native review
settings-integration-search-provider-help = Löydä kuvia kirjastostasi GNOMEn ja KDE:n haulla. Sanat toimivat kuten kirjaston hakupalkissa.
# machine-drafted, needs native review
settings-integration-url-scheme = Avaa specular://-linkit
settings-integration-url-scheme-help = Linkit, kuten specular:///home/me/photo.jpg, avautuvat selaimesta tai muusta ohjelmasta { -app-name }-sovellukseen. Vain tämän tietokoneen tiedostot avataan, ja epätavalliset vasta vahvistuksen jälkeen.

integration-state-on = Käytössä
integration-state-off = Ei käytössä
integration-state-partial = Osittain määritetty. Korjaa käyttämällä uudelleen.
integration-state-unavailable = Ei käytettävissä tällä työpöydällä
integration-state-sandboxed = Ei käytettävissä hiekkalaatikossa (Flatpak)
integration-state-no-bus = Ei käytettävissä: D-Bus-istuntoa ei ole
integration-state-no-file-manager = Ei käytettävissä: tuettua tiedostonhallintaa ei löytynyt
integration-state-no-search = Ei käytettävissä: GNOME Shelliä tai KRunneria ei löytynyt
integration-state-working = Käsitellään…
integration-state-details = { $state } · { $details }
integration-detail-types = kuvatyypit: { $count }/{ $total }
integration-detail-thumbnail-types = kuvatyypit: { $count }
integration-apply = Käytä
integration-remove = Poista
integration-done-on = ”{ $option }” on käytössä
integration-done-off = ”{ $option }” ei ole käytössä
integration-failed = Asetusta ”{ $option }” ei voitu muuttaa: { $reason }
# machine-drafted, needs native review
integration-search-remove-ask = Poistetaanko työpöytähaku käytöstä?
integration-search-remove-detail = { -app-name } poistetaan haun { $desktops } piiristä. Kirjasto ja kuvat säilyvät.
search-provider-open-folder = Avaa kansio

integration-label-open = Avaa { -app-name }issa
integration-label-compare = Vertaa { -app-name }issa
integration-label-wallpaper = Aseta taustakuvaksi
integration-label-generic-name = Kuvankatselin
integration-label-comment = Avaa ja selaa suuria valokuvia viiveettä

palette-set-wallpaper = Aseta taustakuvaksi
wallpaper-set = Taustakuva asetettu ({ $method })
wallpaper-failed = Taustakuvaa ei voitu asettaa: { $reason }

# machine-drafted, needs native review
url-invalid = Tätä { -app-name }-linkkiä ei voi avata: { $reason }
url-not-found = Linkin nimeämiä tiedostoja ei ole olemassa
url-remote-refused = { -app-name } avaa vain tämän tietokoneen tiedostoja, ei kohdetta { $source }
url-confirm-files = { $more ->
    [0] ”{ $file }”
   *[other] ”{ $file }” ja { $more } muuta
}
url-confirm-title = { $reason ->
    [dots] Avataanko { url-confirm-files }? Linkki kulkee ”..”-kansioiden kautta.
    [network] Avataanko { url-confirm-files }? Se on verkkojaossa tai laitteessa.
   *[hidden] Avataanko { url-confirm-files }? Nimessä on näkymättömiä tai tekstin suunnan merkkejä.
}
url-confirm-open = Avaa

# Launcher actions and recent files: machine-drafted, needs native review
integration-label-new-window = Uusi ikkuna
integration-label-library = Kirjasto
integration-label-slideshow = Kuvat-kansion diaesitys
settings-integration-recent-files = Viimeisimmät tiedostot työpöydällä
settings-integration-recent-files-help = Avaamasi kuvat näkyvät tiedostonhallinnan kohdassa ”Viimeisimmät” ja työpöydän viimeisimpien tiedostojen luetteloissa. Poistaminen ottaa { -app-name }-merkinnät taas pois luetteloista.
integration-detail-recent = tiedostoja: { $count }
# machine-drafted, needs native review
integration-extra-title = Lisää tiedostotyyppejä
# machine-drafted, needs native review
integration-extra-intro = Kuvien lisäksi { -app-name } voi avata nämä tiedostot, kun kaksoisnapsautat niitä. Mitään ei ole valittu, ellet itse valitse. Tiedostoja muokkaavat ohjelmat pysyvät kohdassa ”Avaa sovelluksella”, ja valinnan poistaminen palauttaa tyypin aiemmalle ohjelmalle. Blackmagic RAW, DWG, STEP, IGES ja Zeiss CZI tarvitsevat lisäksi valmistajiensa ilmaisen ohjelmiston: katso Asetukset › Valinnaiset dekooderit.
integration-extra-chosen = { $count ->
    [0] Ei valittuja
    [one] 1 tyyppi valittu
   *[other] { $count } tyyppiä valittu
}
integration-extra-count = { $on }/{ $total }
integration-extra-none-yet = Ei vielä yhtään
integration-extra-keywords = tiedostotyypit tiedostomuodot tiedostokytkennät kaksoisnapsautus avaa sovelluksella editori tiede lääketiede CAD GIS paikkatieto 3D molekyylit video SVG PDF
integration-extra-group-editors = Editoritiedostot
# machine-drafted, needs native review
integration-extra-group-editors-help = Maalaus- ja piirto-ohjelmien asiakirjat: Krita, GIMP, Paint.NET, PaintShop Pro, Procreate, Affinity, OpenRaster, Illustrator, CorelDRAW, Clip Studio Paint, Sketch ja Figma. Kaksoisnapsautus näyttää kuvan; muokkaat sitä yhä ohjelmassa, jolla se tehtiin.
integration-extra-group-science = Tiede
integration-extra-group-science-help = Mittalaitteiden ja tutkimusohjelmien tiedostot: lämpökamerat, mikroskoopit, pintamittaukset, HDF5, NetCDF, seismiset ja säätiedot.
integration-extra-group-medical = Lääketiede
integration-extra-group-medical-help = Lääketieteelliset kuvantamistutkimukset ja aivokuvantaminen: DICOM, NIfTI, MINC, NRRD, traktografia ja aivopinnat.
integration-extra-group-cad-gis = CAD ja GIS
# machine-drafted, needs native review
integration-extra-group-cad-gis-help = Piirturipiirustukset, tekniset kuvitukset, piirien layoutit ja kartat: HP-GL, CGM, GDSII, shapefile, GeoPackage, KML, GPX, GeoJSON ja korkeusruudut. DWG avautuu ilmaisella ODA File Converterilla, STEP ja IGES OpenCASCADElla.
integration-extra-group-models = 3D ja molekyylit
integration-extra-group-models-help = 3D-mallit, rakennusmallit, pistepilvet ja molekyylit: STL, OBJ, glTF, PLY, 3MF, X3D, IFC, LAS, E57, PDB, CIF ja muut.
integration-extra-group-videos = Videot
# machine-drafted, needs native review
integration-extra-group-videos-help = Videot: MP4, MOV, M4V, 3GP, 3G2, MKV, WebM, AVCHD (MTS, M2TS), OGV, IVF ja MXF. Blackmagic RAW avautuu ilmaisella Blackmagic RAW SDK:lla.
integration-extra-group-other-images = SVG ja muut kuvat
# machine-drafted, needs native review
integration-extra-group-other-images-help = Kuvat ja asiakirjat, jotka yleensä jätetään muille ohjelmille: SVG-piirrokset, PDF, XPS, DjVu, EPS, WMF ja EMF, Visio, OpenDocument, WordPerfect Graphics, EPUB-kirjat ja fontit sekä animaatiot, kuten MNG, FLIC ja animoidut osoittimet.
integration-extra-group-text = Teksti, taulukot ja tekstitykset
integration-extra-group-text-help = Pelkkä teksti ja lokit, NFO, Markdown, CSV- ja TSV-taulukot, RTF sekä tekstitystiedostot: SRT, WebVTT, ASS, SSA, MicroDVD, PGS, LRC, TTML ja SCC.
