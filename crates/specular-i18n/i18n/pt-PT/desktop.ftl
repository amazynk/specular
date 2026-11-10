# machine-drafted, needs native review
# Integração profunda no ambiente de trabalho: Definições → Integração, definir como imagem de fundo. Português europeu.

settings-section-integration = Integração
settings-integration-default-viewer = Visualizador de imagens predefinido
settings-integration-default-viewer-help = Abrir as imagens com o { -app-name } ao fazer duplo clique. Ao remover esta opção, é reposto o visualizador anterior.
# machine-drafted, needs native review
settings-integration-extra-default-types = Mais tipos de ficheiro
settings-integration-extra-default-types-help = Outros tipos de ficheiro a abrir com o { -app-name } com duplo clique, escolhidos em Visualizador de imagens predefinido.
settings-integration-thumbnailer = Miniaturas no gestor de ficheiros
settings-integration-thumbnailer-help = Permite que os gestores de ficheiros mostrem pré-visualizações de formatos que não conseguem ler sozinhos, como JPEG XL, HEIC e ficheiros RAW de câmara.
settings-integration-single-instance = Instância única através de D-Bus
settings-integration-single-instance-help = Os ficheiros abertos a partir do gestor de ficheiros, de um lançador ou do terminal vão para a janela do { -app-name } já aberta. Aplica-se às janelas abertas a seguir.
settings-integration-file-manager-actions = Ações do gestor de ficheiros
settings-integration-file-manager-actions-help = Adiciona «Abrir», «Comparar» e «Definir como imagem de fundo» aos menus de contexto do Nautilus, Dolphin, Thunar e Nemo.
settings-integration-search-provider = Pesquisa no ambiente de trabalho
# machine-drafted, needs native review
settings-integration-search-provider-help = Encontrar imagens da sua biblioteca na pesquisa do GNOME e do KDE. As palavras funcionam como na barra de pesquisa da biblioteca.
# machine-drafted, needs native review
settings-integration-url-scheme = Abrir ligações specular://
settings-integration-url-scheme-help = Ligações como specular:///home/me/photo.jpg num navegador ou noutro programa abrem no { -app-name }. Só são abertos ficheiros deste computador, e os invulgares só depois de confirmar.

integration-state-on = Ativado
integration-state-off = Desativado
integration-state-partial = Configurado em parte. Aplique novamente para reparar.
integration-state-unavailable = Indisponível neste ambiente de trabalho
integration-state-sandboxed = Indisponível dentro de uma sandbox (Flatpak)
integration-state-no-bus = Indisponível: não existe sessão D-Bus
integration-state-no-file-manager = Indisponível: não foi encontrado nenhum gestor de ficheiros suportado
integration-state-no-search = Indisponível: não foi encontrado o GNOME Shell nem o KRunner
integration-state-working = A processar…
integration-state-details = { $state } · { $details }
integration-detail-types = tipos de imagem: { $count } de { $total }
integration-detail-thumbnail-types = tipos de imagem: { $count }
integration-apply = Aplicar
integration-remove = Remover
integration-done-on = «{ $option }» ativado
integration-done-off = «{ $option }» desativado
integration-failed = Não foi possível alterar «{ $option }»: { $reason }
# machine-drafted, needs native review
integration-search-remove-ask = Desativar a pesquisa no ambiente de trabalho?
integration-search-remove-detail = O { -app-name } é retirado da pesquisa do { $desktops }. A sua biblioteca e as suas imagens ficam.
search-provider-open-folder = Abrir pasta

integration-label-open = Abrir no { -app-name }
integration-label-compare = Comparar no { -app-name }
integration-label-wallpaper = Definir como Imagem de Fundo
integration-label-generic-name = Visualizador de Imagens
integration-label-comment = Abra e percorra fotografias grandes instantaneamente

palette-set-wallpaper = Definir como imagem de fundo
wallpaper-set = Imagem de fundo definida ({ $method })
wallpaper-failed = Não foi possível definir a imagem de fundo: { $reason }

# machine-drafted, needs native review
url-invalid = Não é possível abrir esta ligação do { -app-name }: { $reason }
url-not-found = Os ficheiros indicados por esta ligação não existem
url-remote-refused = O { -app-name } abre apenas ficheiros deste computador, não { $source }
url-confirm-files = { $more ->
    [0] «{ $file }»
   *[other] «{ $file }» e mais { $more }
}
url-confirm-title = { $reason ->
    [dots] Abrir { url-confirm-files }? A ligação chega lá através de pastas «..».
    [network] Abrir { url-confirm-files }? Está numa partilha de rede ou num dispositivo.
   *[hidden] Abrir { url-confirm-files }? O nome contém caracteres invisíveis ou de direção de texto.
}
url-confirm-open = Abrir

# Launcher actions and recent files: machine-drafted, needs native review
integration-label-new-window = Nova janela
integration-label-library = Biblioteca
integration-label-slideshow = Apresentação de Imagens
settings-integration-recent-files = Ficheiros recentes do ambiente de trabalho
settings-integration-recent-files-help = As imagens abertas aparecem em «Recentes» no gestor de ficheiros e nas listas de ficheiros recentes do ambiente de trabalho. Ao remover, as entradas do { -app-name } saem dessas listas.
integration-detail-recent = ficheiros: { $count }
# machine-drafted, needs native review
integration-extra-title = Mais tipos de ficheiro
# machine-drafted, needs native review
integration-extra-intro = Além de imagens, o { -app-name } pode abrir estes ficheiros com duplo clique. Nada fica assinalado até o assinalar. Os programas que editam estes ficheiros continuam em «Abrir com», e desassinalar um tipo devolve-o ao programa que tinha antes. Blackmagic RAW, DWG, STEP, IGES e Zeiss CZI também precisam de software gratuito dos seus fabricantes: veja Definições › Descodificadores opcionais.
integration-extra-chosen = { $count ->
    [0] Nenhum escolhido
    [one] 1 tipo escolhido
    [many] { $count } de tipos escolhidos
   *[other] { $count } tipos escolhidos
}
integration-extra-count = { $on } de { $total }
integration-extra-none-yet = Nenhum ainda
integration-extra-keywords = tipos de ficheiro formatos associações duplo clique abrir com editor ciência médico CAD SIG GIS 3D moléculas vídeo SVG PDF
integration-extra-group-editors = Ficheiros de editores
# machine-drafted, needs native review
integration-extra-group-editors-help = Documentos de programas de pintura e desenho: Krita, GIMP, Paint.NET, PaintShop Pro, Procreate, Affinity, OpenRaster, Illustrator, CorelDRAW, Clip Studio Paint, Sketch e Figma. O duplo clique mostra a imagem; continua a editá-la no programa que a criou.
integration-extra-group-science = Ciência
integration-extra-group-science-help = Ficheiros de instrumentos e software de investigação: câmaras térmicas, microscópios, medições de superfície, HDF5, NetCDF, dados sísmicos e meteorológicos.
integration-extra-group-medical = Médico
integration-extra-group-medical-help = Exames médicos e imagiologia cerebral: DICOM, NIfTI, MINC, NRRD, tractografia e superfícies cerebrais.
integration-extra-group-cad-gis = CAD e SIG
# machine-drafted, needs native review
integration-extra-group-cad-gis-help = Desenhos de plotter, ilustrações técnicas, layouts de chips e mapas: HP-GL, CGM, GDSII, shapefiles, GeoPackage, KML, GPX, GeoJSON e mosaicos de elevação. DWG abre com o ODA File Converter, gratuito, e STEP e IGES com o OpenCASCADE.
integration-extra-group-models = 3D e moléculas
integration-extra-group-models-help = Modelos 3D, modelos de edifícios, nuvens de pontos e moléculas: STL, OBJ, glTF, PLY, 3MF, X3D, IFC, LAS, E57, PDB, CIF e mais.
integration-extra-group-videos = Vídeos
# machine-drafted, needs native review
integration-extra-group-videos-help = Vídeos: MP4, MOV, M4V, 3GP, 3G2, MKV, WebM, AVCHD (MTS, M2TS), OGV, IVF e MXF. Blackmagic RAW abre com o Blackmagic RAW SDK, gratuito.
integration-extra-group-other-images = SVG e outras imagens
# machine-drafted, needs native review
integration-extra-group-other-images-help = Imagens e documentos normalmente deixados a outros programas: desenhos SVG, PDF, XPS, DjVu, EPS, WMF e EMF, Visio, OpenDocument, WordPerfect Graphics, livros EPUB e tipos de letra, e animações como MNG, FLIC e cursores animados.
integration-extra-group-text = Texto, tabelas e legendas
integration-extra-group-text-help = Texto simples e registos, NFO, Markdown, tabelas CSV e TSV, RTF e ficheiros de legendas: SRT, WebVTT, ASS, SSA, MicroDVD, PGS, LRC, TTML e SCC.
