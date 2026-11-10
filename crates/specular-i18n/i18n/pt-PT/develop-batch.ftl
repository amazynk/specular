# machine-drafted, needs native review
# Specular — Develop: histórico, instantâneos, estilos, predefinições, versões, copiar e colar, igualar. Português (Portugal).

palette-develop-copy = Copiar edição
palette-develop-copy-help = Copiar a edição da foto, exceto recorte e lente, para colar em outras fotos.
palette-develop-copy-some = Copiar alguns módulos…
palette-develop-copy-some-help = Escolher quais módulos da edição copiar.
palette-develop-paste = Colar edição
palette-develop-paste-help = Colar a edição copiada na foto, ou nas fotos selecionadas na galeria.
palette-develop-paste-some = Colar alguns módulos…
palette-develop-paste-some-help = Escolher quais módulos copiados colar, a somar ou a substituir.
palette-develop-paste-picks = Colar edição nas escolhidas
palette-develop-paste-picks-help = Colar a edição copiada em cada foto escolhida.
palette-develop-reference = Usar como referência
palette-develop-reference-help = Outras fotos poderão igualar a exposição e o balanço de branco desta.
palette-develop-match = Igualar à referência
palette-develop-match-help = Ajustar a exposição e o balanço de branco da foto, da seleção ou das escolhidas à referência.
palette-develop-sync = Sincronizar com as escolhidas
palette-develop-sync-help = Pôr a edição desta foto, exceto recorte e lente, nas fotos escolhidas.
palette-develop-sync-some = Sincronizar alguns módulos…
palette-develop-sync-some-help = Escolher quais módulos desta foto vão para as fotos escolhidas.
palette-develop-next-capture = Estilo das próximas capturas…
palette-develop-next-capture-help = O estilo que cada ficheiro novo que chega à pasta recebe.
palette-develop-auto-rules = Regras automáticas…
palette-develop-auto-rules-help = As regras que aplicam um estilo na primeira vez que uma foto abre no Develop.
palette-develop-import-style = Importar estilo…
palette-develop-import-style-help = Adicionar um estilo de um ficheiro.
palette-develop-snapshot = Tirar instantâneo
palette-develop-snapshot-help = Dar nome à edição como está agora, para comparar ou voltar a ela depois.
palette-develop-compress = Compactar histórico
palette-develop-compress-help = Tirar passos que outros posteriores substituíram. A edição continua igual.
palette-develop-new-version = Nova versão
palette-develop-new-version-help = Outra edição da mesma foto, com histórico próprio. O ficheiro não é copiado.
palette-develop-new-style = Novo estilo…
palette-develop-new-style-help = Guardar módulos escolhidos da edição como estilo.

develop-styles = Estilos
develop-snapshots = Histórico e instantâneos
develop-versions = Versões
develop-style-new = Novo estilo a partir desta edição…
develop-style-import = Importar estilo…
develop-rules = Regras automáticas…
develop-snapshot-take = Tirar instantâneo
develop-history-compress = Compactar histórico
develop-snapshot-comparing = A comparar: { $name }
develop-snapshot-compare-off = Comparação desligada
develop-version-number = Versão { $number }
develop-version-open = { $name } (aberta)
develop-version-new = Nova versão
develop-version-duplicate = Duplicar esta versão

develop-history-paste = Colar
develop-history-sync = Sincronizar
develop-history-match = Igualar à referência
develop-history-snapshot = Instantâneo: { $name }
develop-history-previous = Igual à anterior

develop-rename = Renomear…
develop-delete = Eliminar
develop-save = Guardar
develop-name = Nome
develop-style-apply = Aplicar
develop-style-apply-replace = Aplicar substituindo estes módulos
develop-style-auto = Aplicar a fotos novas desta câmara
develop-style-export = Exportar…
develop-snapshot-restore = Voltar a este instantâneo
develop-version-open-it = Abrir
develop-preset-save = Guardar como predefinição…
develop-preset-save-module = Guardar { $module } como predefinição…
develop-preset-delete = Eliminar “{ $name }”…

develop-sheet-modules = Módulos
develop-sheet-included = Incluído
develop-sheet-left-out = Deixado de fora
develop-sheet-more = mais { $count }
develop-copy-title = Copiar módulos
develop-copy-ok = Copiar
develop-paste-title = Colar módulos
develop-paste-ok = Colar
develop-paste-mode = Módulos que já estão na edição
develop-paste-append = Somar
develop-paste-replace = Substituir
develop-style-new-title = Novo estilo
develop-style-save = Guardar estilo
develop-style-default-name = Meu estilo { $number }
develop-style-rename-title = Renomear estilo
develop-preset-save-title = Predefinição de { $module }
develop-preset-default-name = Minha predefinição
develop-snapshot-take-title = Tirar instantâneo
develop-snapshot-default-name = Instantâneo { $number }
develop-snapshot-rename-title = Renomear instantâneo
develop-version-new-title = Nova versão
develop-version-duplicate-title = Duplicar versão
develop-version-create = Criar
develop-version-rename-title = Renomear versão

develop-style-delete-ask = Eliminar o estilo “{ $name }”?
develop-style-delete-detail = As fotos em que foi aplicado mantêm a edição. O ficheiro do estilo é removido.
develop-preset-delete-ask = Eliminar a predefinição “{ $name }”?
develop-preset-delete-detail = As edições em que foi aplicada ficam como estão.
develop-snapshot-delete-ask = Eliminar o instantâneo “{ $name }”?
develop-snapshot-delete-detail = A edição e o histórico ficam como estão.
develop-version-delete-ask = Eliminar “{ $name }”?
develop-version-delete-detail = A edição e o histórico desta versão são removidos. A foto e as outras versões continuam no disco.
develop-rule-delete-ask = Eliminar a regra “{ $rule }”?
develop-rule-delete-detail = O estilo fica, e as fotos já abertas mantêm a edição.

develop-rule-text = { $style }: { $when }
develop-rule-every-photo = todas as fotos
develop-rule-iso-min = ISO { $iso } ou mais
develop-rule-iso-max = ISO até { $iso }
develop-rule-iso-range = ISO { $low }–{ $high }
develop-rule-raw = ficheiros raw
develop-rule-other = não raw
develop-rule-any-camera = qualquer câmara
develop-next-placeholder = Estilo de cada ficheiro novo
develop-next-off-row = Nenhum
develop-next-previous-row = Igual à foto anterior

develop-copied = { $count ->
    [one] 1 módulo copiado de { $name }
   *[other] { $count } módulos copiados de { $name }
}
develop-pasted = { $count ->
    [one] Colado em 1 foto
   *[other] Colado em { $count } fotos
}
develop-nothing-to-copy = Nenhuma foto de onde copiar.
develop-nothing-copied = Nada copiado ainda. Copie uma edição primeiro (Ctrl+Alt+C).
develop-no-photos = Nenhuma foto para mudar.
develop-no-picks = Nenhuma foto escolhida.
develop-reference-set = Referência: { $name }
develop-no-reference = Ainda sem referência. Abra a foto de referência e escolha Usar como referência (Ctrl+Alt+R).
develop-reference-unreadable = Não foi possível ler a foto de referência.
develop-matching = { $count ->
    [one] A igualar 1 foto…
   *[other] A igualar { $count } fotos…
}
develop-matched = { $count ->
    [one] 1 foto igualada
   *[other] { $count } fotos igualadas
}
develop-matched-some = { $count } igualadas. { $failed } não puderam ser lidas ou medidas.
develop-style-saved = Estilo guardado: { $name }
develop-style-applied = Estilo aplicado: { $name }
develop-style-imported = Estilo importado: { $name }
develop-style-not-a-style = Este ficheiro não é um estilo.
develop-style-exported = Estilo exportado: { $name }
develop-style-export-failed = Não foi possível gravar o estilo ali.
develop-style-import-dialog = Importar um estilo
develop-style-export-dialog = Exportar o estilo
develop-style-file-kind = Estilo
develop-preset-saved = Predefinição guardada: { $name }
develop-preset-applied = Predefinição aplicada: { $name }
develop-snapshot-taken = Instantâneo criado: { $name }
develop-history-compressed = { $count ->
    [0] Nada para compactar
    [one] 1 passo tirado
   *[other] { $count } passos tirados
}
develop-version-created = Versão criada: { $name }
develop-version-opened = Versão aberta: { $name }
develop-version-deleted = Versão eliminada: { $name }
develop-rule-added = { $name } se aplica a fotos novas de { $camera }.
develop-rule-applied = Regra automática: { $name }
develop-rules-none = Nenhuma regra automática. Adicione uma pelo menu de um estilo.
develop-next-off = Ficheiros novos não recebem estilo.
develop-next-previous = Ficheiros novos recebem a edição da foto anterior.
develop-next-style = Ficheiros novos recebem o estilo { $name }.
develop-next-applied = { $count ->
    [one] Estilo das próximas capturas em 1 ficheiro novo
   *[other] Estilo das próximas capturas em { $count } ficheiros novos
}

develop-style-specular = Specular marcante
develop-style-soft = Suave
develop-style-vivid = Vívido
develop-style-mono = Preto e branco
develop-preset-plus-one = Mais claro (+1 EV)
develop-preset-minus-one = Mais escuro (−1 EV)
develop-preset-low-contrast = Contraste baixo
develop-preset-high-contrast = Contraste alto
develop-preset-open-shadows = Abrir sombras
develop-preset-crisp = Nítido
develop-preset-muted = Suavizado
develop-preset-high-iso = ISO alto
develop-preset-screen = Para ecrãs

gallery-badge-versions = { $count } versões
# machine-drafted, needs native review
develop-batch-row = A guardar edições
# machine-drafted, needs native review
develop-batch-row-detail = { $done } de { $total } fotografias
# machine-drafted, needs native review
develop-batch-saving = A guardar
# machine-drafted, needs native review
develop-batch-stopping = A parar
# machine-drafted, needs native review
develop-batch-stopped = Parado. Guardadas { $done } de { $total } fotografias; as restantes não mudaram.
