extends Node3D

## The input, driven by hand: the mouse turns the shot, S draws the cue back,
## E moves the tip, and the shot is on when the cue comes home.
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

## Where the mouse is.
var mouse := Vector2(640, 360)

## The aim before the mouse moves.
var first_aim := Vector3.ZERO


func _ready() -> void:
	game = preload("res://scripts/main.gd").new()
	add_child(game)
	await get_tree().process_frame
	await get_tree().process_frame

	first_aim = game.base_aim
	await _test_the_mouse_turns_the_shot()
	await _test_the_mouse_is_absolute()
	await _test_the_english()
	await _test_the_draw_does_not_squirt()
	await _test_the_release_cancels()
	await _test_the_draw_and_the_stroke()
	_report()


## The mouse turns the shot, the way it turns the view in a first person game.
func _test_the_mouse_turns_the_shot() -> void:
	_move_by(Vector2(40, 0))
	await get_tree().process_frame
	await get_tree().process_frame
	var turned: float = game.base_aim.distance_to(first_aim)
	print("the mouse turned the shot by ", turned, " rad")
	if turned < 0.01:
		printerr("the mouse did not turn the shot")


## And the shot stays where the mouse put it.
func _test_the_mouse_is_absolute() -> void:
	var aim: Vector3 = game.base_aim
	# A small move, and then nothing.
	_move_by(Vector2(6, 0))
	await get_tree().process_frame
	await get_tree().process_frame
	var settled: Vector3 = game.base_aim
	for step in 20:
		await get_tree().process_frame
	var drift: float = game.base_aim.distance_to(settled)
	print("the shot drifted by ", drift, " rad")
	if drift > 0.002:
		printerr("the shot drifted: it carried on moving by ", drift, " rad")


## E moves the tip on the ball, and the shot is left alone.
func _test_the_english() -> void:
	var aim: Vector3 = game.base_aim
	_hold(KEY_E)
	_move_by(Vector2(40, 0))
	await get_tree().process_frame
	_release(KEY_E)
	print("the tip is at ", game.contact)
	if game.contact.x <= 0.0:
		printerr("the english did not move the tip to the shooter's right")
	if game.base_aim.distance_to(aim) > 0.001:
		printerr("the english moved the shot as well")
	# An off-centre tip makes the ball squirt away from the side that was
	# struck, and the preview has to show the line the ball really takes: a
	# preview that shows the cue's line is a promise the shot does not keep.
	var squirted: Vector3 = game._shot_direction()
	print("the ball leaves at ", squirted, " and the cue points at ", game._aim())
	if squirted.distance_to(game._aim()) < 0.0001:
		printerr("the preview does not show the squirt")


## A tip above or below the centre gives follow or draw, and no squirt at all.
func _test_the_draw_does_not_squirt() -> void:
	var saved: Vector2 = game.contact
	game.contact = Vector2(0.0, 0.010)
	var squared: float = game._shot_direction().distance_to(game._aim())
	print("a follow leaves at ", game._shot_direction(), " and the cue points at ", game._aim())
	if squared > 0.0001:
		printerr("a follow squirted by {squared} rad")
	game.contact = saved


## Letting go of the key without a stroke puts the game back to the default.
func _test_the_release_cancels() -> void:
	_hold(KEY_S)
	_move_by(Vector2(0, 60))
	await get_tree().process_frame
	_release(KEY_S)
	await get_tree().process_frame
	print("the phase after letting go is ", game.phase)
	if game.phase != 0:
		printerr("the game did not go back to the default: the phase is ", game.phase)
	if game.pulled != 0.0 or game.draw != 0.0:
		printerr("the draw was not forgotten: ", game.pulled, " ", game.draw)


## S draws the cue back, and pushing it forward shoots.
func _test_the_draw_and_the_stroke() -> void:
	_hold(KEY_S)
	# The cue is drawn back: the mouse goes down the screen, towards the player.
	_move_by(Vector2(0, 200))
	await get_tree().process_frame
	print("the cue is drawn back ", game.pulled, " m")
	if game.pulled <= 0.001:
		printerr("the cue was not drawn back")
	# And the stroke: the mouse comes back up, and the cue comes home and the
	# shot goes.
	_move_by(Vector2(0, -300))
	await get_tree().process_frame
	await get_tree().process_frame
	print("the phase is ", game.phase)
	# The cue travels into the ball, and then the shot is resolved.
	var limit := 0
	while game.shot == null and limit < 200:
		limit += 1
		await get_tree().process_frame
	if game.shot == null:
		printerr("the shot did not fire")
	# And letting go of the key now only ever cancels: the shot is on already.
	_release(KEY_S)
	await get_tree().process_frame
	if game.shot == null:
		printerr("the shot was cancelled by letting go of the key")


## Puts a mouse event into the engine.
func _move_by(relative: Vector2) -> void:
	mouse += relative
	var motion := InputEventMouseMotion.new()
	motion.position = mouse
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
