# DICOM export: anonymised copies, patient disc folders and burned-in text masks.

palette-export-anonymised = Exportar cópia anonimizada…
palette-export-patient-disc = Exportar pasta de disco do paciente…
palette-mask-burned-in-text = Mascarar texto gravado
anon-title = Exportar cópia anonimizada
anon-disc-title = Exportar pasta de disco do paciente
anon-images = Imagens
anon-images-current = Esta imagem
anon-images-series = { $count ->
    [one] Esta série ({ $count } arquivo)
   *[other] Esta série ({ $count } arquivos)
}
anon-images-study = { $count ->
    [one] Este estudo ({ $count } arquivo)
   *[other] Este estudo ({ $count } arquivos)
}
anon-images-reading = Lendo a pasta…
anon-anonymise = Anonimizar
anon-dates = Datas e horas
anon-dates-remove = Remover
anon-dates-keep = Manter
anon-dates-shift = Deslocar
anon-patient = Características do paciente
anon-device = Identidade do equipamento
anon-institution = Identidade da instituição
anon-descriptions = Descrições
anon-descriptions-remove = Remover
anon-descriptions-clean = Limpar
anon-structured = Conteúdo dos laudos
anon-uids = UIDs
anon-uids-replace = Substituir
anon-uids-keep = Manter
anon-overlays = Sobreposições e curvas
anon-keep = Manter
anon-remove = Remover
anon-on = Ativado
anon-off = Desativado
anon-burned-in = Texto gravado
anon-masks = { $count ->
    [one] { $count } área mascarada
   *[other] { $count } áreas mascaradas
}
anon-masks-none = Sem máscaras
anon-folder = Pasta
anon-folder-choose = Escolher…
anon-export = Exportar
anon-risk-present = Estas imagens indicam que mostram texto do paciente: mascare-o antes de exportar.
anon-risk-possible = Imagens deste tipo costumam mostrar texto do paciente: verifique antes de exportar.
anon-progress = Exportando { $done } de { $total }…
anon-stopping = Parando…
anon-mask-title = Mascarar texto gravado
anon-mask-hint = Arraste sobre o texto para mascará-lo. As áreas mascaradas ficam pretas em todos os quadros.
anon-mask-suggest = Sugerir
anon-mask-remove = Remover
anon-mask-clear = Limpar
anon-mask-done = Concluído
anon-mask-area = Área { $n }, { $width } × { $height } pixels
anon-mask-suggested = { $count ->
    [0] Nenhum texto encontrado perto das bordas
    [one] { $count } área de texto sugerida
   *[other] { $count } áreas de texto sugeridas
}
anon-done = { $count ->
    [one] { $count } arquivo anonimizado gravado em { $folder }
   *[other] { $count } arquivos anonimizados gravados em { $folder }
}
anon-disc-done = { $count ->
    [one] Pasta de disco do paciente com { $count } imagem gravada em { $folder }
   *[other] Pasta de disco do paciente com { $count } imagens gravada em { $folder }
}
anon-skipped-burned-in = { $count ->
    [one] { $count } arquivo com texto gravado e sem máscaras foi ignorado
   *[other] { $count } arquivos com texto gravado e sem máscaras foram ignorados
}
anon-skipped-failed = { $count ->
    [one] Não foi possível exportar { $count } arquivo
   *[other] Não foi possível exportar { $count } arquivos
}
anon-stopped = Exportação interrompida
anon-failed = Não foi possível exportar: { $reason }
anon-not-empty = A pasta já contém um disco do paciente
