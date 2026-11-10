# machine-drafted, needs native review
# Derin masaüstü tümleştirmesi: Ayarlar → Tümleştirme, duvar kâğıdı olarak ayarla.

settings-section-integration = Tümleştirme
settings-integration-default-viewer = Varsayılan görüntü görüntüleyici
settings-integration-default-viewer-help = Görüntülere çift tıkladığınızda { -app-name } ile açılsın. Bunu kaldırmak önceki görüntüleyiciyi geri getirir.
# machine-drafted, needs native review
settings-integration-extra-default-types = Diğer dosya türleri
settings-integration-extra-default-types-help = Çift tıklandığında { -app-name } ile açılacak diğer dosya türleri; Varsayılan görüntü görüntüleyici altında seçilir.
settings-integration-thumbnailer = Dosya yöneticisi küçük resimleri
settings-integration-thumbnailer-help = Dosya yöneticilerinin kendilerinin okuyamadığı JPEG XL, HEIC ve kamera RAW dosyaları gibi biçimlerin önizlemelerini göstermesini sağlar.
settings-integration-single-instance = D-Bus üzerinden tek örnek
settings-integration-single-instance-help = Dosya yöneticisinden, bir başlatıcıdan veya uçbirimden açılan dosyalar çalışan { -app-name } penceresine gider. Bundan sonra açılan pencerelere uygulanır.
settings-integration-file-manager-actions = Dosya yöneticisi eylemleri
settings-integration-file-manager-actions-help = Nautilus, Dolphin, Thunar ve Nemo bağlam menülerine “Aç”, “Karşılaştır” ve “Duvar kâğıdı olarak ayarla” ekler.
settings-integration-search-provider = Masaüstü araması
# machine-drafted, needs native review
settings-integration-search-provider-help = Kitaplığınızdaki resimleri GNOME ve KDE aramasında bulun. Sözcükler kitaplığın arama çubuğundaki gibi çalışır.
# machine-drafted, needs native review
settings-integration-url-scheme = specular:// bağlantılarını aç
settings-integration-url-scheme-help = Tarayıcıda veya başka bir programdaki specular:///home/me/photo.jpg gibi bağlantılar { -app-name } içinde açılır. Yalnızca bu bilgisayardaki dosyalar açılır; alışılmadık olanlar ancak onayınızdan sonra.

integration-state-on = Açık
integration-state-off = Kapalı
integration-state-partial = Kısmen kurulu. Onarmak için yeniden uygulayın.
integration-state-unavailable = Bu masaüstünde kullanılamıyor
integration-state-sandboxed = Korumalı alan (Flatpak) içinde kullanılamıyor
integration-state-no-bus = Kullanılamıyor: D-Bus oturumu yok
integration-state-no-file-manager = Kullanılamıyor: desteklenen bir dosya yöneticisi bulunamadı
integration-state-no-search = Kullanılamıyor: ne GNOME Shell ne de KRunner bulundu
integration-state-working = Çalışıyor…
integration-state-details = { $state } · { $details }
integration-detail-types = görüntü türleri: { $count } / { $total }
integration-detail-thumbnail-types = görüntü türleri: { $count }
integration-apply = Uygula
integration-remove = Kaldır
integration-done-on = “{ $option }” açık
integration-done-off = “{ $option }” kapalı
integration-failed = Değiştirilemedi: “{ $option }”: { $reason }
# machine-drafted, needs native review
integration-search-remove-ask = Masaüstü araması kapatılsın mı?
integration-search-remove-detail = { -app-name }, { $desktops } aramasından çıkarılır. Kitaplığınız ve resimleriniz kalır.
search-provider-open-folder = Klasörü aç

integration-label-open = { -app-name } ile aç
integration-label-compare = { -app-name } ile karşılaştır
integration-label-wallpaper = Duvar Kâğıdı Olarak Ayarla
integration-label-generic-name = Görüntü Görüntüleyici
integration-label-comment = Büyük fotoğrafları anında açın ve göz atın

palette-set-wallpaper = Duvar kâğıdı olarak ayarla
wallpaper-set = Duvar kâğıdı ayarlandı ({ $method })
wallpaper-failed = Duvar kâğıdı ayarlanamadı: { $reason }

# machine-drafted, needs native review
url-invalid = Bu { -app-name } bağlantısı açılamıyor: { $reason }
url-not-found = Bu bağlantının belirttiği dosyalar yok
url-remote-refused = { -app-name } yalnızca bu bilgisayardaki dosyaları açar, { $source } değil
url-confirm-files = { $more ->
    [0] “{ $file }”
   *[other] “{ $file }” ve { $more } tane daha
}
url-confirm-title = { $reason ->
    [dots] { url-confirm-files } açılsın mı? Bağlantı ona “..” klasörleri üzerinden ulaşıyor.
    [network] { url-confirm-files } açılsın mı? Bir ağ paylaşımında veya aygıtta.
   *[hidden] { url-confirm-files } açılsın mı? Adında görünmez veya metin yönü karakterleri var.
}
url-confirm-open = Aç

# Launcher actions and recent files: machine-drafted, needs native review
integration-label-new-window = Yeni Pencere
integration-label-library = Kitaplık
integration-label-slideshow = Resimler slayt gösterisi
settings-integration-recent-files = Masaüstündeki son dosyalar
settings-integration-recent-files-help = Açtığınız görüntüler dosya yöneticisinde “Son” altında ve masaüstünün son dosyalar listelerinde görünür. Kaldırıldığında { -app-name } girdileri bu listelerden yeniden çıkarılır.
integration-detail-recent = dosyalar: { $count }
# machine-drafted, needs native review
integration-extra-title = Diğer dosya türleri
# machine-drafted, needs native review
integration-extra-intro = { -app-name }, resimlerin yanı sıra bu dosyaları da çift tıklandığında açabilir. Siz işaretlemedikçe hiçbiri işaretli değildir. Bu dosyaları düzenleyen programlar “Birlikte aç” içinde kalır; bir türün işaretini kaldırmak onu önceki programına geri verir. Blackmagic RAW, DWG, STEP, IGES ve Zeiss CZI ayrıca üreticilerinin ücretsiz yazılımını gerektirir: Ayarlar › İsteğe bağlı kod çözücüler bölümüne bakın.
integration-extra-chosen = { $count ->
    [0] Hiçbiri seçilmedi
    [one] 1 tür seçildi
   *[other] { $count } tür seçildi
}
integration-extra-count = { $on }/{ $total }
integration-extra-none-yet = Henüz yok
integration-extra-keywords = dosya türleri biçimler ilişkilendirme çift tıklama birlikte aç düzenleyici bilim tıp CAD CBS GIS 3D moleküller video SVG PDF
integration-extra-group-editors = Düzenleyici dosyaları
# machine-drafted, needs native review
integration-extra-group-editors-help = Boyama ve çizim programlarının belgeleri: Krita, GIMP, Paint.NET, PaintShop Pro, Procreate, Affinity, OpenRaster, Illustrator, CorelDRAW, Clip Studio Paint, Sketch ve Figma. Çift tıklama resmi gösterir; düzenlemeyi yine onu oluşturan programda yaparsınız.
integration-extra-group-science = Bilim
integration-extra-group-science-help = Cihaz ve araştırma yazılımı dosyaları: termal kameralar, mikroskoplar, yüzey ölçümleri, HDF5, NetCDF, sismik ve hava durumu verileri.
integration-extra-group-medical = Tıp
integration-extra-group-medical-help = Tıbbi taramalar ve beyin görüntüleme: DICOM, NIfTI, MINC, NRRD, traktografi ve beyin yüzeyleri.
integration-extra-group-cad-gis = CAD ve CBS
# machine-drafted, needs native review
integration-extra-group-cad-gis-help = Çizici çizimleri, teknik illüstrasyonlar, yonga yerleşimleri ve haritalar: HP-GL, CGM, GDSII, shapefile, GeoPackage, KML, GPX, GeoJSON ve yükseklik karoları. DWG ücretsiz ODA File Converter ile, STEP ve IGES ise OpenCASCADE ile açılır.
integration-extra-group-models = 3D ve moleküller
integration-extra-group-models-help = 3D modeller, bina modelleri, nokta bulutları ve moleküller: STL, OBJ, glTF, PLY, 3MF, X3D, IFC, LAS, E57, PDB, CIF ve daha fazlası.
integration-extra-group-videos = Videolar
# machine-drafted, needs native review
integration-extra-group-videos-help = Videolar: MP4, MOV, M4V, 3GP, 3G2, MKV, WebM, AVCHD (MTS, M2TS), OGV, IVF ve MXF. Blackmagic RAW ücretsiz Blackmagic RAW SDK ile açılır.
integration-extra-group-other-images = SVG ve diğer görüntüler
# machine-drafted, needs native review
integration-extra-group-other-images-help = Genellikle başka programlara bırakılan resimler ve belgeler: SVG çizimleri, PDF, XPS, DjVu, EPS, WMF ve EMF, Visio, OpenDocument, WordPerfect Graphics, EPUB kitapları ve yazı tipleri ile MNG, FLIC ve animasyonlu imleçler gibi animasyonlar.
integration-extra-group-text = Metin, tablolar ve altyazılar
integration-extra-group-text-help = Düz metin ve günlükler, NFO, Markdown, CSV ve TSV tabloları, RTF ve altyazı dosyaları: SRT, WebVTT, ASS, SSA, MicroDVD, PGS, LRC, TTML ve SCC.
