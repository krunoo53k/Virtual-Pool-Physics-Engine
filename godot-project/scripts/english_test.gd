extends Node3D

## The english the player reads: the game tells the tip offset and the side
## the ball will leave on, the same numbers the probes measure by hand, and
## the tip comes home between the shots.
##
##     godot --headless --path godot-project scenes/english_test.tscn
##
## The exit code is 0 when the status told the truth and the tip came home,
## 1 when it did not.

const ROOT := preload("res://scripts/main.gd")

var game: Node3D
var fails := 0


func _ready() -> void:
	game = ROOT.new()
	add_child(game)
	game.mouse_turns_the_shot = false
	await get_tree().process_frame
	await get_tree().process_frame

	# A tip on the shooter's right: the status quotes it in millimetres, and
	# the ball leaves the other way.
	game.contact = Vector2(0.010, 0.0)
	await get_tree().process_frame
	var status: String = game.status.text
	print("status: ", status)
	_expect(status.contains("10 mm right")) # the tip, in the player's words
	_expect(status.contains("leaves left by 1.9")) # the squirt, to the other side

	# A tip above the centre: follow, and a straight shot.
	game.contact = Vector2(0.0, 0.005)
	await get_tree().process_frame
	status = game.status.text
	print("status: ", status)
	_expect(status.contains("5 mm above"))
	_expect(status.contains("leaves straight"))

	# The debug panel: **D** raises it, and the panel names the shot in the
	# same numbers the status gives: the line of the cue, the line the ball
	# really leaves on, the tip and the ghost.
	game.debug = true
	game.contact = Vector2(0.010, 0.0)
	await get_tree().process_frame
	var panel: String = game.debug_panel.text
	print("the panel says: ", panel)
	_expect(panel.contains("10 mm right"))
	_expect(panel.contains("deg left"))
	_expect(panel.contains("the ghost  ball"))

	# The tip comes home at the end of the shot, so the next stroke is a
	# centre-ball stroke.
	game.fire_now(9.0)
	var limit := 20000
	while game.phase != game.Phase.AIMING and limit > 0:
		limit -= 1
		await get_tree().process_frame
	var tip: Vector2 = game.contact
	print("after the shot the tip is at ", tip)
	_expect(tip == Vector2.ZERO)
	_expect(game.status.text.contains("E for the english") and not game.status.text.contains("tip "))

	print("fails: ", fails)
	get_tree().quit(0 if fails == 0 else 1)


func _expect(ok: bool) -> void:
	if not ok:
		fails += 1
		printerr("  the expectation failed")
