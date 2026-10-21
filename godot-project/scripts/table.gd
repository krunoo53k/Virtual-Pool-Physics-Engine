class_name PoolTable
extends RefCounted

## The table, built from the core's own geometry.
##
## The rails, the pockets and the cushion come from `PoolCore.get_table_geometry`,
## so the table the player looks at is the table the balls roll on: the
## cushion stops where the model says the pocket begins, because the spans of
## a rail end at the jaw tips.
##
## The geometry is in the core's frame - `x` along the table, `y` across it,
## `z` up - and the node this builds is in the same frame, so that the balls
## can be put straight into it.

## How wide a rail is, in metres. The model does not care: the cushion is a
## line, and the wood behind it is only there to look at.
const RAIL_WIDTH := 0.05

## How thick the rubber of the cushion is, in metres.
const CUSHION_DEPTH := 0.012

## How wide a pocket is drawn, in metres. It is a dark disc on the cloth: the
## hole itself is under the bed, where nobody can see it.
const POCKET_DEPTH := 0.09

## The colour of the cloth.
const CLOTH := Color(0.09, 0.32, 0.18)

## The colour of the rails.
const WOOD := Color(0.32, 0.19, 0.10)

## The colour of the rubber.
const RUBBER := Color(0.07, 0.13, 0.09)

## The colour of the inside of a pocket.
const POCKET := Color(0.05, 0.05, 0.06)

## Builds the table.
static func build(core: PoolCore) -> Node3D:
	var geometry: Dictionary = core.get_table_geometry()
	var table := Node3D.new()
	table.name = "Table"

	# The bed: the cloth on top, at the height the balls rest on.
	var length: float = geometry["length"]
	var width: float = geometry["width"]
	table.add_child(_box(
		Vector3(length, 0.04, width),
		Vector3(length / 2.0, -0.02, -width / 2.0),
		CLOTH))

	# The rails, one box for every span of every rail: the spans stop at the
	# jaw tips, so the pocket mouths are open.
	var rail_height: float = geometry["cushion"]["rail_top_height"]
	for rail in geometry["rails"]:
		var normal := Vector2(rail["normal"][0], rail["normal"][1])
		var tangent := Vector2(rail["tangent"][0], rail["tangent"][1])
		var line: float = rail["line"]
		for span in rail["spans"]:
			var from: float = span[0]
			var to: float = span[1]
			# The nose of the cushion is on the line, and the rail runs away
			# from the table along the normal.
			var along := (from + to) / 2.0
			var centre := Vector2(
				tangent.x * along + normal.x * (line + RAIL_WIDTH / 2.0),
				tangent.y * along + normal.y * (line + RAIL_WIDTH / 2.0))
			var size := Vector3(0.0, 0.0, 0.0)
			if absf(tangent.x) > 0.5:
				size = Vector3(absf(to - from), rail_height, RAIL_WIDTH)
			else:
				size = Vector3(RAIL_WIDTH, rail_height, absf(to - from))
			table.add_child(_box(
				size,
				Vector3(centre.x, rail_height / 2.0, -centre.y),
				WOOD))
			# The rubber on the nose of the cushion.
			var rubber_size := size
			if absf(tangent.x) > 0.5:
				rubber_size = Vector3(absf(to - from), rail_height, CUSHION_DEPTH)
			else:
				rubber_size = Vector3(CUSHION_DEPTH, rail_height, absf(to - from))
			# The rubber is on the nose of the cushion: the inner edge of
			# the rail, and the middle of the span.
			var rubber_centre := Vector3(
				centre.x - normal.x * (RAIL_WIDTH / 2.0 - CUSHION_DEPTH / 2.0),
				rail_height / 2.0,
				-(centre.y - normal.y * (RAIL_WIDTH / 2.0 - CUSHION_DEPTH / 2.0)))
			table.add_child(_box(rubber_size, rubber_centre, RUBBER))

	# The pockets: a dark hole in the cloth, at the mouth of each pocket.
	for pocket in geometry["pockets"]:
		var centre: Array = pocket["mouth_centre"]
		var mouth: float = pocket["width"]
		var hole := MeshInstance3D.new()
		var cylinder := CylinderMesh.new()
		cylinder.top_radius = mouth / 2.0
		cylinder.bottom_radius = mouth / 2.0
		cylinder.height = POCKET_DEPTH * 0.05
		cylinder.radial_segments = 24
		hole.mesh = cylinder
		var material := StandardMaterial3D.new()
		material.albedo_color = POCKET
		material.roughness = 0.9
		hole.material_override = material
		hole.position = Vector3(centre[0], 0.002, -centre[1])
		table.add_child(hole)
	return table

## A box with a colour.
static func _box(size: Vector3, centre: Vector3, colour: Color) -> MeshInstance3D:
	var mesh := MeshInstance3D.new()
	var box := BoxMesh.new()
	box.size = size
	mesh.mesh = box
	var material := StandardMaterial3D.new()
	material.albedo_color = colour
	material.roughness = 0.75
	mesh.material_override = material
	mesh.position = centre
	return mesh
