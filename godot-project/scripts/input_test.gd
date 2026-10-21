extends Node3D

## The input, driven by hand: the mouse aims, S draws the cue back, a flick
## lets it go.
##
## The events are put into the engine the way the keyboard and the mouse put
## them in, so the game cannot tell the difference - which is the point: the
## game is driven by nothing but its input.
##
##     godot --headless --path godot-project scenes/input_test.tscn
##
## The exit code is 0 when the input did what it says, 1 when it did not.

## The game under test.
var game: Node3D

## The aim before the mouse moves.
var first_aim := Vector3.ZERO


func _ready() -> void:
	game = preload("res://scripts/main.gd").new()
	add_child(game)
	await get_tree().process_frame
	await get_tree().process_frame

	first_aim = game.base_aim
	await _test_the_aim()
	await _test_the_english()
	await _test_the_draw_and_the_flick()
	_report()


## A mouse that moves turns the aim.
func _test_the_aim() -> void:
	_move_mouse(Vector2(200, 200), Vector2(0, 0))
	await get_tree().process_frame
	await get_tree().process_frame
	var moved: float = game.base_aim.distance_to(first_aim)
	print("the mouse turned the aim by ", moved)
	if moved < 0.001:
		printerr("the mouse did not turn the aim")


## E moves the tip on the cue ball.
func _test_the_english() -> void:
	_hold(KEY_E)
	_move_mouse(Vector2(300, 300), Vector2(40, 0))
	await get_tree().process_frame
	_release(KEY_E)
	print("the tip is at ", game.contact)
	if game.contact.x <= 0.0:
		printerr("the english did not move the tip to the shooter's right")


## S draws the cue back, and the flick lets it go.
func _test_the_draw_and_the_flick() -> void:
	_hold(KEY_S)
	# The cue is drawn back: the mouse goes away from the ball, along the aim.
	# The screen points are worked out through the camera, so that the mouse
	# goes where a hand would put it.
	var ball: Vector3 = game.balls[0].position
	var behind: Vector3 = ball - game.base_aim * 0.25
	var on_the_ball: Vector2 = game.camera.unproject_position(ball)
	var behind_the_ball: Vector2 = game.camera.unproject_position(behind)
	_move_mouse(behind_the_ball, behind_the_ball - on_the_ball)
	await get_tree().process_frame
	print("the cue is drawn back ", game.pulled, " m")
	if game.pulled <= 0.001:
		printerr("the cue was not drawn back")
	if game.pulled > 0.301:
		printerr("the cue was drawn back ", game.pulled, " m, which is too far")
	# And the flick: the mouse comes forward again, fast.
	for step in 4:
		var forward: Vector2 = (on_the_ball - behind_the_ball) * 0.4
		_move_mouse(behind_the_ball + forward, forward)
		await get_tree().process_frame
	print("the flick is ", game.flick, " m/s")
	_release(KEY_S)
	# The cue travels into the ball first, and then the shot is resolved.
	var limit := 0
	while game.shot == null and limit < 200:
		limit += 1
		await get_tree().process_frame
	print("the phase is ", game.phase)
	if game.shot == null:
		printerr("the shot did not fire")


## Puts a mouse event into the engine.
func _move_mouse(position: Vector2, relative: Vector2) -> void:
	var motion := InputEventMouseMotion.new()
	motion.position = position
	motion.relative = relative
	Input.parse_input_event(motion)


## Holds a key down.
func _hold(keycode: Key) -> void:
	var event := InputEventKey.new()
	event.keycode = keycode
	event.physical_keycode = keycode
	event.pressed = true
	Input.parse_input_event(event)


## Lets a key go.
func _release(keycode: Key) -> void:
	var event := InputEventKey.new()
	event.keycode = keycode
	event.physical_keycode = keycode
	event.pressed = false
	Input.parse_input_event(event)


## Reports what happened.
func _report() -> void:
	var ok := game.shot != null
	if ok:
		print("the input fired a shot of ", game.shot.get_duration(), " s")
	else:
		printerr("the input did not fire a shot")
	get_tree().quit(0 if ok else 1)
