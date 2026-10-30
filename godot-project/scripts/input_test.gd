extends Node3D

## The input, driven by hand: the mouse turns the shot, S draws the cue back,
## B raises the cue, E moves the tip, F trims the turn finer, and the shot is
## on when the cue comes home.
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

## Whether one of the checks said no. The checks the file already had only
## ever printed to the error stream; the new ones set this as well, and the
## exit code carries it, so a jump shot that never jumped is an exit code and
## not only a line in a log.
var failed := false


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
	await _test_the_cue_is_raised()
	await _test_the_fine_trim()
	await _test_the_jump_shot()
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
	# The stroke is taken: the aim is what the player looked at, and the shot
	# that goes must be that aim. The flick that takes the stroke is a big,
	# fast drag, and its sideways wobble used to turn the aim as well - the
	# deviation came out proportional to the force of the flick, which is
	# what the player keeps reporting. A mouse that settles after the flick,
	# while the cue travels or the balls roll, must now leave the aim where
	# the player left it.
	var aim_the_player_saw: Vector3 = game.base_aim
	# The cue travels into the ball, and then the shot is resolved. The mouse
	# wobbles sideways on the way: a few frames of travel with the hand
	# still settling.
	var limit := 0
	while game.shot == null and limit < 200:
		if limit == 3:
			_move_by(Vector2(120, 0))
		if limit == 6:
			_move_by(Vector2(-90, 0))
		limit += 1
		await get_tree().process_frame
	if game.shot == null:
		printerr("the shot did not fire")
	# The balls are still rolling the frame the shot resolves: a mouse that
	# settles during the roll must not move the aim either.
	_move_by(Vector2(-60, 0))
	await get_tree().process_frame
	await get_tree().process_frame
	# The shot that goes is the one the player looked at: the flick that
	# took the stroke and the mouse settling after it must have left the
	# aim where the player left it.
	var wobble: float = game.base_aim.distance_to(aim_the_player_saw)
	print("the aim wobbled by ", wobble, " rad once the mouse settled")
	if wobble > 1e-6:
		# The aim turned after the stroke was taken: the shot that goes is
		# not the one the panel showed, and by an amount that grows with
		# the force of the flick.
		printerr("the aim turned by ", wobble, " rad after the stroke was taken: the shot is not the one the player looked at")
	# And letting go of the key now only ever cancels: the shot is on already.
	_release(KEY_S)
	await get_tree().process_frame
	if game.shot == null:
		printerr("the shot was cancelled by letting go of the key")


## B raises the cue, and the core has to feel it: while B is held the mouse's
## up and down lifts the butt of the cue, which points the shaft down at the
## ball. The raised cue is what makes a stroke a jump shot (§7.1's jump is
## the slate launching a ball the cue drove down), so the raise has to reach
## the stroke the core is given and not only the drawing: the test reads the
## core's own reading of the cue's height out of _cue_words, which asks the
## core for it, and a raise that never reached the stroke would leave those
## words calling the cue level.
func _test_the_cue_is_raised() -> void:
	# A raise is only taken while aiming: once the cue is drawn back the
	# mouse's up and down belongs to the draw, and the test before this
	# one left the game mid-shot. Waiting is what puts the raise on the
	# aim instead of dropping it on the floor.
	await _wait_until_aiming()
	# The refused ghost only refuses a ball the cloth was going to
	# promise, so the line has to name one first. The tests broke the
	# rack, and turning until a ball is on the line is the look a player
	# takes after a break.
	await _turn_until_the_line_names_a_ball()
	_check(game.aim_has_hit, "the level aim named no ball: the refusal has nothing to refuse")
	# Where the line stood before the raise, to hold the raise to: the cue
	# rises, and the line it rises on does not move.
	var line: Vector3 = game._level_aim()
	_hold(KEY_B)
	# 200 px down the screen at ANGLE_PER_PIXEL is 20 deg: the same drag a
	# player makes against the window's edge, far past where the ghost is
	# refused and nowhere near MAX_CUE_ANGLE.
	_move_by(Vector2(0.0, 200.0))
	await get_tree().process_frame
	await get_tree().process_frame
	# The raise arrived at the aim: the direction the core is handed points
	# below the cloth. Down-screen raises the butt, so the shaft points
	# down at the ball, the way a first-person pitch reads.
	_check(game.cue_angle > 0.0, "holding B and dragging down did not raise the cue (cue_angle %.3f)" % game.cue_angle)
	_check(game._aim().y < 0.0, "the raised cue did not point below the cloth (aim y %.6f)" % game._aim().y)
	_check(game._level_aim().distance_to(line) <= 1e-6, "the raise moved the line the mouse turned")
	# The ghost is refused past PREVIEW_MAX_ANGLE: the hop a raised stroke
	# makes meets the next ball in the air, so the picture stops promising
	# where the ball goes, while the ball stays named on the line.
	_check(not game.aim_has_hit, "the ghost still promised a hop at %.1f deg" % game.cue_angle)
	_check(game.aim_hit_ball >= 0, "the refused ghost lost the ball it was about to refuse")
	# And the raise is in the core's own words: _cue_words asks the core
	# for the height of the very stroke the panel is promising, so this is
	# the proof the height reached the physics and not only the drawing.
	_check(game._cue_words() != "the cue is level", "the core still called the raised cue level")
	# The same drag back up, still under B, puts the ghost back where it
	# was: the refusal belongs to the height, and not to a switch that
	# stays thrown once it is thrown.
	_move_by(Vector2(0.0, -200.0))
	_release(KEY_B)
	await get_tree().process_frame
	await get_tree().process_frame
	_check(absf(game.cue_angle) < 1e-6, "the cue did not come back level (cue_angle %.3f)" % game.cue_angle)
	_check(game.aim_has_hit, "the ghost did not come back once the cue was level")
	_check(game._cue_words() == "the cue is level", "the core did not call the levelled cue level again")


## F trims the turn: the same 40 px turns the aim a quarter as far with F
## held as without (FINE_SCALE in _mouse_motion), and the trim turns itself
## back out after, so the next test starts where this one started.
func _test_the_fine_trim() -> void:
	await _wait_until_aiming()
	# The plain turn first: 40 px is the turn the other tests make, and
	# the angle it makes is what the trim is measured against.
	var before: Vector3 = game._level_aim()
	_move_by(Vector2(40.0, 0.0))
	await get_tree().process_frame
	await get_tree().process_frame
	var coarse: float = before.angle_to(game._level_aim())
	_move_by(Vector2(-40.0, 0.0))
	await get_tree().process_frame
	await get_tree().process_frame
	# The same 40 px with F held: a quarter of the turn, small but the
	# same share the trim always takes, and not some other number.
	before = game._level_aim()
	_hold(KEY_F)
	_move_by(Vector2(40.0, 0.0))
	_release(KEY_F)
	await get_tree().process_frame
	await get_tree().process_frame
	var fine: float = before.angle_to(game._level_aim())
	_check(coarse > 1e-4, "the plain mouse would not turn (turned %.6f rad)" % coarse)
	_check(fine > 0.0, "F held down would not trim the turn at all")
	# Vector3 counts in 32-bit floats, so the angle a rotated vector reads
	# back carries a mote of dust from the normalizing: a mote, not a
	# number, and far under any turn the trim could take the blame for.
	_check(absf(fine - coarse * game.FINE_SCALE) < 1e-6, "F trimmed the turn to %.9f rad where the plain turn went %.9f: that is not FINE_SCALE" % [fine, coarse])
	# The trim turns itself back out: the aim is back where the plain
	# turn left it, and the next test looks from there.
	_hold(KEY_F)
	_move_by(Vector2(-40.0, 0.0))
	_release(KEY_F)
	await get_tree().process_frame
	await get_tree().process_frame


## The jump: E puts the tip under the centre of the ball, B raises the cue
## above the cloth, and the stroke at break speed drives the ball into the
## slate, which launches it (§7.1). The end of the path cannot say it flew -
## the slate's rebound brings the ball back to the cloth and it rests where
## it lands - so the flight is read on the path with get_ball_state_at, the
## sampling the renderer plays the path with.
func _test_the_jump_shot() -> void:
	await _wait_until_aiming()
	await _turn_until_the_line_names_a_ball()
	_check(game.aim_has_hit, "the jump had no ball on the line to happen to")
	# The tip below the centre first: 41 px down the screen at SPIN_SCALE
	# is 0.01148 m, a shade under half a radius below the centre, the
	# stroke the jump takes (the miscue limit keeps the tip on the face,
	# and a tip over the centre drives the ball into the cloth instead).
	_hold(KEY_E)
	_move_by(Vector2(0.0, 41.0))
	_release(KEY_E)
	await get_tree().process_frame
	_check(game.contact.y < -game.ball_radius * 0.3, "the tip did not go below the centre (y %.5f m)" % game.contact.y)
	# The cue steep next: 400 px down at ANGLE_PER_PIXEL is 40 deg, far
	# past where the ghost is refused and well inside MAX_CUE_ANGLE.
	_hold(KEY_B)
	_move_by(Vector2(0.0, 400.0))
	_release(KEY_B)
	await get_tree().process_frame
	_check(game.cue_angle >= 40.0, "the cue was not raised steep enough to jump (%.1f deg)" % game.cue_angle)
	# The stroke, at break speed, taken the way play_test takes its
	# strokes: the path is the core's own, and the height of the cue is
	# inside it before the first ball is drawn.
	game.shot = null
	game.fire_now(9.0)
	for i in range(600):
		if game.shot != null:
			break
		await get_tree().process_frame
	_check(game.shot != null, "the jump shot never reached the core")
	# The read: the cue ball has to leave the cloth somewhere on its path,
	# and higher than a radius, or the raise that was driven into the
	# stroke never made a jump of it. 10 ms steps; the hop of any jump
	# worth the name is far longer than one step.
	var flight := false
	var top := 0.0
	var duration: float = game.shot.get_duration()
	var seconds := 0.0
	while seconds < duration:
		var state: Dictionary = game.shot.get_ball_state_at(0, seconds)
		if String(state["motion_state"]) == "airborne":
			flight = true
			var height: float = state["position"].y
			top = maxf(top, height)
		seconds += 0.01
	_check(flight, "the cue ball never left the cloth: the raised cue did not jump it")
	_check(top > game.ball_radius, "the hop never cleared a ball radius (top %.5f m)" % top)
	# The end of the path, printed for the log: it reads at rest even on a
	# shot that flew, which is the whole reason the flight is read on the
	# path and not off the last state.
	var rested: Dictionary = game.shot.get_ball_state_at(0, duration)
	print("the jump topped out at %.4f m and ended reading %s" % [top, String(rested["motion_state"])])


## One check: a no is a line in the log and the exit code, and the run goes
## on, so one bad check cannot hide the ones after it.
func _check(ok: bool, words: String) -> void:
	if ok:
		return
	failed = true
	printerr(words)


## Waits for the game to come back to the aim - Phase.AIMING is the enum's
## first entry, stored as its number - because everything the mouse does to
## the cue is only read while aiming, and a test that skipped the wait
## would be testing the floor the dropped events landed on.
func _wait_until_aiming() -> void:
	for i in range(4000):
		if game.phase == 0:
			return
		await get_tree().process_frame
	failed = true
	printerr("the game never came back to the aim")


## Turns the mouse a dot at a time until the line of the aim names a ball,
## the look a player takes after a break. The tests have broken the rack, and
## where the balls the breaks left standing is nobody's promise, so the look
## goes right first and then far past where it started to the left: 4 px to a
## step is finer than the window the farthest ball on the table can subtend,
## so a step cannot step over a ball it would have found.
func _turn_until_the_line_names_a_ball() -> void:
	for i in range(1500):
		if game.aim_hit_ball >= 0:
			return
		_move_by(Vector2(4.0 if i < 300 else -4.0, 0.0))
		await get_tree().process_frame
	failed = true
	# Where the balls actually stand, for the log: a sweep that found
	# nothing is a diagnosis it can make of itself.
	var words := "no ball ever came onto the line of the aim; the cue stands at %s, the aim looks along %s, and %d live ball(s) stand at" % [game._vector(game.balls[0].position), game._vector(game.base_aim), game.live.size()]
	for ball_index in game.live:
		if ball_index == 0:
			continue
		words += " (%.2f, %.2f)" % [game.balls[ball_index].position.x, game.balls[ball_index].position.z]
	printerr(words)


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
	var ok := game.shot != null and not failed
	if ok:
		print("the input fired a shot of ", game.shot.get_duration(), " s")
	else:
		if failed:
			printerr("a check said no: see the lines above")
		printerr("the input did not fire a shot")
	get_tree().quit(0 if ok else 1)
