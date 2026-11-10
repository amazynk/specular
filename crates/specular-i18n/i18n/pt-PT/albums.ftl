# machine-drafted, needs native review
# Specular — Library albums, smart albums and the date timeline.

## Sidebar
albums-title = Álbuns
albums-new = Novo álbum…
albums-new-smart = Novo álbum inteligente…
albums-save-search = Guardar a pesquisa como álbum inteligente…
albums-smart = Álbum inteligente
# The album the photos go to when added without choosing.
albums-target = As adições vão para aqui
albums-group-count = { $count ->
    [one] { $count } álbum
   *[other] { $count } álbuns
}
albums-need-library = Os álbuns estão na Biblioteca. Adicione primeiro esta pasta à Biblioteca.

## Screen readers: the buttons of album and group rows
albums-a11y-rename = Mudar o nome de { $album }
albums-a11y-edit = Editar { $album }
albums-a11y-delete = Eliminar o álbum { $album }
albums-a11y-rename-group = Mudar o nome do grupo { $group }
albums-a11y-delete-group = Eliminar o grupo { $group }

## Sheets
albums-name = Nome
albums-unnamed = Ainda sem nome
albums-new-title = Novo álbum
albums-new-smart-title = Novo álbum inteligente
albums-edit-smart-title = Editar álbum inteligente
albums-rename-title = Mudar o nome do álbum
albums-rename-group-title = Mudar o nome do grupo
albums-create = Criar
albums-rename = Mudar o nome
albums-save = Guardar
albums-groups-note = Use / para o pôr num grupo: Cliente X / seleção.
albums-name-empty = Escreva um nome.
albums-name-taken = { $album } já existe.
albums-name-clash = { $album } é um álbum, por isso não pode ser também um grupo.

## Smart album rules
albums-rules = Regras
albums-rules-note = As regras aplicam-se de cima para baixo. As regras vazias são ignoradas.
albums-rules-none = Adicione pelo menos uma regra.
albums-rule-label = Regra { $number }
albums-rule-empty = Regra vazia
albums-rule-add = Adicionar uma regra
albums-rule-combine = Combinar com as regras acima
albums-rule-first = Fotografias que cumprem
albums-rule-and = E
albums-rule-or = Ou
albums-rule-except = Exceto

## Adding and removing photos
albums-add-menu = Adicionar ao álbum…
albums-remove-menu = Remover do álbum
albums-add-title = { $count ->
    [one] Adicionar { $count } fotografia a um álbum
   *[other] Adicionar { $count } fotografias a um álbum
}
albums-add = Adicionar
albums-add-new = Novo álbum
albums-add-list = Álbuns
albums-add-none = Ainda não há álbuns. Escreva um nome para criar um.
albums-add-note = Ou escreva um nome para um novo álbum.
albums-added = { $count ->
    [one] { $count } fotografia adicionada a { $album }.
   *[other] { $count } fotografias adicionadas a { $album }.
}
albums-added-missing = { $count ->
    [one] { $count } não está na Biblioteca, por isso não foi adicionada.
   *[other] { $count } não estão na Biblioteca, por isso não foram adicionadas.
}
albums-created = { $album } criado.
albums-removed = { $count ->
    [one] { $count } fotografia removida de { $album }.
   *[other] { $count } fotografias removidas de { $album }.
}
albums-remove = Remover
albums-remove-confirm = { $count ->
    [one] Remover { $count } fotografia de { $album }?
   *[other] Remover { $count } fotografias de { $album }?
}
albums-remove-confirm-detail = Os ficheiros ficam no disco e na Biblioteca.
albums-delete = Eliminar
albums-delete-confirm = Eliminar o álbum { $album }?
albums-delete-group-confirm = { $count ->
    [one] Eliminar o grupo { $group } e o seu { $count } álbum?
   *[other] Eliminar o grupo { $group } e os seus { $count } álbuns?
}
albums-delete-confirm-detail = As fotografias ficam no disco e na Biblioteca.
albums-deleted = { $count ->
    [one] Álbum eliminado.
   *[other] { $count } álbuns eliminados.
}
albums-failed = Não foi possível alterar os álbuns: { $reason }

## Date timeline
albums-timeline = Cronologia
albums-timeline-description = Escolha um período para mostrar só as suas fotografias. Shift+clique escolhe vários seguidos.
albums-timeline-picked = A mostrar { $period }
albums-timeline-cleared = A mostrar todas as datas
