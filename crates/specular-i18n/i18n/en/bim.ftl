## Reviewing 3D models: walk and fly, section planes and box, saved views, BCF topics
## (mesh mode). Keys named here are the defaults.

cheatsheet-review = Review

palette-mesh-walk = Walk
palette-mesh-fly = Fly
palette-mesh-views = Saved views
palette-mesh-save-view = Save the view
palette-mesh-section-plane = Add a section plane
palette-mesh-section-box = Section box
palette-mesh-section-flip = Flip the section plane
palette-mesh-section-clear = Remove all cuts
palette-mesh-measure-cut = Measure on the cut
palette-mesh-bcf-topics = BCF topics
palette-mesh-model-checks = Model checks
palette-mesh-models = Models in the scene
palette-mesh-add-model = Add model…
palette-mesh-add-model-folder = Add models from a folder…

# Walking and flying: toasts and readout lines, by $what. $eye: the eye's height over the
# floor, with its unit (walk-line only).
mesh-nav = { $what ->
    [walk-toast] Walking: W A S D to move, drag to look, Esc to stop
    [fly-toast] Flying: W A S D to move, Q E down and up, Esc to stop
    [orbit] Orbit
    [walk-line] Walking · eye { $eye } over the floor
    [walk-keys] W A S D move · drag looks · Shift faster · Esc stops
    [preparing] Walls not ready yet: you pass through them
    [fly-keys] W A S D move · Q E down, up · drag looks · Shift faster · Esc stops
   *[fly-line] Flying
}

# Short texts of cuts, measuring and saved views, by $what.
bim-note = { $what ->
    [plane-added] Section plane added: drag its handle to move it
    [box-on] Section box on: drag a face's handle to move it
    [box-off] Section box off
    [cleared] Cuts removed
    [measure-on] Click two points on the cut
    [measure-needs-cut] Add a section plane or the box first
    [measure-off-cut] Click on the cut
    [measure-hint] Measuring: click two points on the cut
    [ask-view-name] Name of the view
    [ask-rename] New name
    [views-empty] No saved views. Press N to save this one.
    [views-hints] Enter go · N save · F2 rename · Delete remove · P save as PNG
    [delete-detail] Only the saved view goes. The model is not changed.
    [delete-ok] Remove
    [not-over-model] Choose another file: this would replace the model.
   *[view-name] View
}

# BCF (BIM Collaboration Format) short texts, by $what.
bcf-note = { $what ->
    [title-none] BCF topics
    [new-file] new
    [empty] No topics. Press O to open a BCF file or N to add a topic.
    [hints] Enter go · ← → viewpoints · N topic · C comment · S status · E title · O open · Ctrl+S export
    [ask-title] Title of the topic
    [ask-comment] Comment
    [untitled] Untitled
    [topic-added] Topic added with this view
   *[comment-added] Comment added with this view
}

# $planes: how many section planes.
bim-sections-line = { $planes ->
    [0] No section planes
    [one] 1 section plane
   *[other] { $planes } section planes
}
# $size: the section box's size with its unit.
bim-box-line = box { $size }
# $count: the most planes there can be.
bim-planes-full = At most { $count } section planes
# A drag handle, for screen readers. $n: its number.
bim-handle = { $kind ->
    [box] Section box face { $n }
   *[plane] Section plane { $n }
}
# $length: the length with its unit.
bim-measure-line = Length on the cut: { $length }
bim-views-title = { $count ->
    [one] 1 saved view
   *[other] { $count } saved views
}
# $name: a view's name.
bim-view-saved = Saved “{ $name }”
bim-view-delete-title = Remove the view “{ $name }”?
# $file: a file written.
bim-saved-file = Saved { $file }
# $reason: why it failed.
bim-failed = { $what ->
    [open] Could not open the BCF file: { $reason }
    [views] Views kept for this session only: { $reason }
   *[save] Could not save: { $reason }
}
# $count: topics; $file: the archive's file name.
bcf-title = { $count ->
    [one] 1 topic
   *[other] { $count } topics
} · { $file }
bcf-comments = { $count ->
    [one] 1 comment
   *[other] { $count } comments
}
# $at: the viewpoint shown; $count: the topic's viewpoints.
bcf-viewpoint-of = Viewpoint { $at } of { $count }
bcf-opened = { $count ->
    [one] 1 topic opened
   *[other] { $count } topics opened
}

## Several IFC models in one scene (the Models list, Alt+Shift+M).

# The list's title. $count: models in the scene.
fed-title = { $count ->
    [one] Models: 1
   *[other] Models: { $count }
}
fed-note = { $what ->
    [not-ifc] This file is not an IFC model.
    [not-ifc-open] Open an IFC model first. Only IFC models can be combined.
    [this-model] This model
    [hints] A add model · Shift+A add folder · Space show or hide · C colour by model · Enter go · Delete remove
    [origin-ok] Place at origin
    [first-stays] The model you opened stays. Open another file to change it.
    [remove-detail] It leaves the scene. The file on disk stays.
   *[remove-ok] Remove
}
# A model's row. $basis: how it was placed; $elements: its elements; $shown: shown or hidden.
fed-row = { $basis } · { $elements ->
    [one] 1 element
   *[other] { $elements } elements
} · { $shown ->
    [hidden] hidden
   *[shown] shown
}
# How a model was placed against the first. $crs: the coordinate system's name.
fed-basis = { $basis ->
    [reference] opened model
    [map-crs] by map coordinates ({ $crs })
    [map] by map coordinates
    [site] by site latitude and longitude (approximate)
    [project] by shared project coordinates
   *[origin] at the origin (not aligned)
}
# Why a model cannot be aligned. $first, $this: coordinate system names.
fed-unaligned = { $why ->
    [crs] The models use different coordinate systems: { $first } and { $this }.
    [only-first] The opened model is georeferenced, this one is not.
    [only-this] This model is georeferenced, the opened one is not.
   *[broken] This model's map conversion cannot be used (its scale is zero).
}
# The chosen model's placement. $distance: how far it moved; $angle: degrees turned.
fed-placed = Moved { $distance }, turned { $angle }°
fed-adding = { $count ->
    [one] Reading 1 model…
   *[other] Reading { $count } models…
}
fed-ask-origin = { $file } can't be aligned. Place it at the origin?
fed-no-files = No IFC files in { $folder }.
fed-not-added = { $file } was not added: { $reason }
fed-added = { $file } added. { $count ->
        [one] { $count } model in the scene.
       *[other] { $count } models in the scene.
    }
fed-remove-title = Remove { $file } from the scene?
fed-removed = { $file } removed from the scene.
fed-shown = { $file } { $shown ->
    [hidden] hidden
   *[shown] shown
}
fed-colours = { $on ->
    [on] Colour by model on
   *[off] Colour by model off
}

## PMI (product manufacturing information) of a CAD model: dimensions, geometric tolerances,
## datums and the file's saved views (mesh mode).

palette-mesh-pmi = PMI and saved views
# $count: annotations in the model.
pmi-title = { $count ->
    [one] 1 annotation
   *[other] { $count } annotations
}
pmi-view = Saved view
pmi-view-all = shows all annotations
# $count: annotations the saved view shows.
pmi-view-shows = { $count ->
    [one] shows 1 annotation
   *[other] shows { $count } annotations
}
# $name: the saved view's name.
pmi-view-shown = Showing view “{ $name }”. A shows all.
pmi-has-faces = faces lit
# Short texts, by $what.
pmi-note = { $what ->
    [none] No PMI in this model.
    [hints] ↑ ↓ choose · Enter go · A all · D as drawn · Shift+A hide
    [hidden] Annotations hidden
    [shown] Annotations shown
    [as-drawn] Annotations as the file draws them
    [as-labels] Annotations as labels
   *[all-shown] All annotations shown
}
# What an annotation is, by $what.
pmi-kind = { $what ->
    [dimension] Dimension
    [tolerance] Geometric tolerance
    [datum] Datum
    [target] Datum target
   *[graphic] Drawn annotation
}
pmi-dimension = { $what ->
    [linear] Distance
    [angular] Angle
    [diameter] Diameter
    [radius] Radius
    [length] Length
    [thickness] Thickness
   *[other] Dimension
}
# Geometric characteristics (ISO 1101), by $what.
pmi-characteristic = { $what ->
    [straightness] Straightness
    [flatness] Flatness
    [circularity] Circularity
    [cylindricity] Cylindricity
    [line-profile] Profile of a line
    [surface-profile] Profile of a surface
    [angularity] Angularity
    [perpendicularity] Perpendicularity
    [parallelism] Parallelism
    [position] Position
    [concentricity] Concentricity
    [coaxiality] Coaxiality
    [symmetry] Symmetry
    [circular-runout] Circular runout
   *[total-runout] Total runout
}
