class_name PoolRack
extends RefCounted

## The eight-ball rack: the balls on the table before the break.
##
## The apex ball is on the foot spot, which is a quarter of the length of the
## table from the foot rail, and the rack runs towards the foot rail, so the
## cue ball breaks into the apex. The eight is in the middle of the third
## row, and the corners of the back row are one from each group.

## The rack: the positions of the sixteen balls and their numbers.
##
## The first ball is the cue ball; the other fifteen are the rack.
static func rack(core: PoolCore) -> Dictionary:
	var geometry: Dictionary = core.get_table_geometry()
	var length: float = geometry["length"]
	var width: float = geometry["width"]
	var radius: float = geometry["ball_radius"]
	var foot := Vector3(length / 4.0, width / 2.0, radius)

	var positions: Array[Vector3] = [Vector3(3.0 * length / 4.0, width / 2.0, radius)]
	var numbers: Array[int] = [0]
	var index := 0
	# The rows of the triangle: one ball, then two, and so on.
	for row in 5:
		var x := foot.x - sqrt(3.0) * radius * row
		for column in row + 1:
			var y := foot.y + (2.0 * column - row) * radius
			positions.append(Vector3(x, y, radius))
			numbers.append(_NUMBERS[index])
			index += 1
	return {"positions": positions, "numbers": numbers}

## The ball numbers, in the order they go into the rack.
##
## The corners of the back row take one from each group, so that a break
## cannot leave one of the groups off the table.
const _NUMBERS := [
	1,
	9, 2,
	10, 8, 3,
	11, 12, 4, 13,
	5, 14, 6, 15, 7,
]
