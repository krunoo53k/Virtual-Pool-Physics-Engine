extends SceneTree

## Does the ghost the game draws match where the ball really meets?
##
## Sets up a grazing shot the way the game would, then compares:
##   * the preview line the game draws,
##   * the ghost ball position from `_first_contact`,
##   * the actual ball-ball collision from the core.
##
##     godot --headless --path godot-project --script res://scripts/ghost_probe.gd

const R := 0.028575
const CUE := Vector3(0.90, R, -0.635)
const AHEAD := 1.40


func _init() -> void:
	var core := PoolCore.new()
	print("=== the ghost against the real collision ===")
	for dz in [0.020, 0.040, 0.050, 0.055]:
		_ghost_against_reality(core, dz)
	quit(0)


func _ghost_against_reality(core: PoolCore, dz: float) -> void:
	var aim := Vector3(1, 0, 0)
	var positions := PackedVector3Array()
	positions.append(CUE)
	positions.append(CUE + Vector3(AHEAD, 0.0, dz))
	var numbers := PackedInt32Array([0, 1])

	# What the game draws: the squirty line and the ghost on it.
	var preview: Dictionary = core.preview_cue_strike(3.0, aim, Vector2.ZERO)
	var direction: Vector3 = preview["velocity"].normalized()
	var best := INF
	var found := Vector3.ZERO
	for index in [1]:
		var ball: Vector3 = positions[index]
		var to_ball := ball - CUE
		var along := to_ball.dot(aim)
		var across := (to_ball - aim * along).length()
		if along > 0.0 and across <= 2.0 * R:
			best = along
			var back := sqrt(maxf(0.0, 4.0 * R * R - across * across))
			found = CUE + aim * (along - back)

	# What the shot does.
	var shot := core.simulate(3.0, aim, Vector2.ZERO, positions, numbers)
	var impact := Vector3.INF
	var hit := false
	for event in shot.get_events():
		if event["kind"] == "ball-ball":
			hit = true
			var at: PackedVector3Array = shot.sample(event["time"])
			impact = at[0]
			break
	print("dz=%.4f  hit=%s" % [dz, hit])
	if hit:
		var error := impact.distance_to(found)
		print("  ghost at %s, collision at %s, apart by %.6f m (%.2f ball radii)" % [
			found, impact, error, error / R])


