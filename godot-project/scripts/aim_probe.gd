extends SceneTree

## A probe for the aim: where does the cue ball really go?
##
## Fires shots through the extension and reads the path back, so the answer
## does not depend on what the renderer draws.
##
##     godot --headless --path godot-project --script res://scripts/aim_probe.gd
##
## The script is a SceneTree, so --script can run it directly.

const R := 0.028575


func _init() -> void:
	var core := PoolCore.new()

	# --- A: a thin cut, no english. Does the ball meet the object ball? ---
	print("=== A: thin cuts, no english ===")
	for dz in [0.030, 0.045, 0.050, 0.052, 0.054, 0.055, 0.056, 0.0565, 0.0570]:
		_thin_cut(core, dz)

	# --- B: the actual initial velocity against the aim, no english ---
	print("=== B: initial velocity against the aim, no english ===")
	_deviation(core, Vector2.ZERO)

	# --- C: right english: where does the ball go, and does the preview say so? ---
	print("=== C: english ===")
	for x in [0.005, 0.010, 0.014]:
		print("-- right english, tip at +", x, " m")
		_deviation(core, Vector2(x, 0.0))
		print("-- left english, tip at -", x, " m")
		_deviation(core, Vector2(-x, 0.0))

	# --- D: full dump of the dead-straight shot ---
	print("=== D: the whole of the dead-straight shot ===")
	_dump(core)

	quit(0)


## A cue ball on the head spot aiming along +x, an object ball ahead by `ahead`
## and across by `dz`, in Godot's frame. The cue ball sits on the centre line
## of the table, the way the game racks it, and not on a rail.
func _positions(ahead: float, dz: float) -> PackedVector3Array:
	var p := PackedVector3Array()
	p.append(Vector3(0.90, R, -0.635))
	p.append(Vector3(0.90 + ahead, R, -0.635 + dz))
	return p


## How far ahead the object ball sits: 1.4 m, inside the table.


## The initial velocity of the cue ball, read from the path.
func _first_velocity(shot: PoolShot) -> Vector3:
	var t := 1e-4
	return (shot.sample(t)[0] - Vector3(0.90, R, -0.635)) / t


func _thin_cut(core: PoolCore, dz: float) -> void:
	var aim := Vector3(1, 0, 0)
	var shot := core.simulate(3.0, aim, Vector2.ZERO, _positions(1.4, dz), PackedInt32Array([0, 1]))
	var events: Array = shot.get_events()
	var hit := false
	var t := -1.0
	for event in events:
		if event["kind"] == "ball-ball":
			hit = true
			t = event["time"]
	print("dz=%.4f  hit=%s  first event t=%s" % [dz, hit, t])


## Dumps the whole event list and the ball positions over time.
func _dump(core: PoolCore) -> void:
	var aim := Vector3(1, 0, 0)
	var shot := core.simulate(3.0, aim, Vector2.ZERO, _positions(1.4, 0.0), PackedInt32Array([0, 1]))
	print("duration: ", shot.get_duration(), " s, error: '", shot.get_error(), "'")
	for event in shot.get_events():
		print("  event t=%s kind=%s a=%s b=%s detail=%s" % [event["time"], event["kind"], event["a"], event["b"], event["detail"]])
	print("  final state:")
	var final := shot.get_final_state()
	for ball in final["balls"]:
		print("    ball %s at %s, pocketed %s" % [ball["number"], ball["position"], ball["pocketed"]])
	var times := [0.0, 0.2, 0.5, 0.8, 1.0, 1.5, 2.0]
	for t in times:
		var p: PackedVector3Array = shot.sample(t)
		print("  t=%s  ball0 at %s  ball1 at %s" % [t, p[0], p[1]])


func _deviation(core: PoolCore, contact: Vector2) -> void:
	var aim := Vector3(1, 0, 0)
	var shot := core.simulate(3.0, aim, contact, _positions(1.4, 0.0), PackedInt32Array([0, 1]))
	var v := _first_velocity(shot)
	# The shooter's right in Godot's frame, for an aim along +x, is +z.
	var right := aim.cross(Vector3.UP).normalized()
	var forward := aim
	var along := v.dot(forward)
	var across := v.dot(right)
	var angle := atan2(across, along)
	# And what the preview promised.
	var preview: Dictionary = core.preview_cue_strike(3.0, aim, contact)
	var pv: Vector3 = preview["velocity"]
	var p_along := pv.dot(forward)
	var p_across := pv.dot(right)
	print("  trace: across %+.6f m/s (%+.4f deg), speed %.3f" % [across, angle * 57.29578, v.length()])
	print("  preview: across %+.6f m/s (%+.4f deg), squirt_deg %+.4f" % [p_across, atan2(p_across, p_along) * 57.29578, preview["squirt_deg"]])
	var events: Array = shot.get_events()
	var t := -1.0
	for event in events:
		if event["kind"] == "ball-ball":
			t = event["time"]
	print("  dead straight ahead, hit at t=%s" % t)
