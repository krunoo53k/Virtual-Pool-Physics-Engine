extends Node3D

## The game: the aim, the cue, the shot and the path.
##
## The physics is not here. The core resolves a shot in one call and gives
## back the path of every ball; this script assembles the input, plays the
## path back and draws the table.
##
## The controls, the way a pool game has always done it:
##
## * the mouse turns the shot, the way it turns the view in a first person
##   game - the view is locked level, and the shot is the only thing the
##   mouse turns;
## * **S** draws the cue back, and pushing the mouse forward shoots;
## * **E** holds the english: the mouse then moves the tip on the cue ball;
## * **B** holds the fine aim: the mouse then turns the shot more slowly.
##
## While **E** is held the mouse moves the tip on the ball instead of turning
## the shot, so the two never fight.

enum Phase { AIMING, CHARGING, STRIKING, ROLLING }

## The helpers. They are loaded by hand rather than by their class name, so
## that the project runs without the editor having built its class cache.
const PoolBall := preload("res://scripts/ball.gd")
const PoolRack := preload("res://scripts/rack.gd")
const PoolTable := preload("res://scripts/table.gd")

## How far the cue can be drawn back, in metres.
const MAX_PULL := 0.30

## The speed of the cue at a full draw, in metres per second.
const MAX_CUE_SPEED := 9.0

## How long the cue takes to travel into the ball, in seconds.
const STRIKE_TIME := 0.06

## How fast the mouse turns the shot, in radians per pixel. A first person
## camera turns with the mouse, not towards it.
const LOOK_SPEED := 0.0035

## How much finer the aim is while **B** is held.
const FINE_SCALE := 0.25

## How far the cue is drawn back per pixel of mouse, in metres.
const DRAW_SPEED := 0.002

## How far the tip moves on the ball per pixel of mouse, in metres.
const SPIN_SCALE := 0.00028

## The physics.
var core: PoolCore

## The shot being played back.
var shot: PoolShot

## What the game is doing.
var phase: int = Phase.AIMING

## The balls.
var balls: Array[PoolBall] = []

## The balls that are still on the table, by their index in `balls`. The cue
## ball is the first one, because the core strikes the first ball it is given.
var live: Array[int] = []

## Where the cue ball starts: the head spot.
var cue_spot := Vector3.ZERO

## The balls that left the table in the last shot, by their index in `balls`.
var off_table: Array[int] = []

## The balls that went into a pocket in the last shot, by their index in `balls`.
var pocketed: Array[int] = []

## The cue stick.
var cue: MeshInstance3D

## The line the cue ball takes.
var aim_line: MeshInstance3D

## The ball the cue ball is about to hit.
var ghost: MeshInstance3D

## The mark on the cue ball where the tip strikes.
var spin_marker: MeshInstance3D

## Whether the cue ball has a ball to hit on the line of the shot. The ghost
## ball is only drawn when it has.
var aim_has_hit := false

## The camera the player looks through.
var camera: Camera3D

## The camera of the table view: the one **C** shows.
##
## It never moves. The shot is turned by the mouse and the view follows the
## shot, so nothing here feeds back into the shot - which is what keeps the
## mouse from behaving like a joystick.
var table_camera: Camera3D

## Which view the camera is in: 0 is the table, 1 is down the cue.
var camera_mode := 1

## How far behind the cue ball the cue camera sits, in metres.
var cue_camera_back := 0.62

## How high above the bed the cue camera sits, in metres.
var cue_camera_height := 0.155

## How far ahead of the cue ball the cue camera looks, in metres.
var cue_camera_ahead := 2.4

## Where the table camera sits, in the table's frame.
var table_camera_offset := Vector3(0.0, 1.42, -1.72)

## How wide the camera sees, in degrees.
var camera_fov := 54.0

## The middle of the table, in the core's frame.
var table_middle := Vector3(1.27, 0.0, -0.635)

## The direction of the shot. The mouse turns it, the way it turns the view
## in a first person game: relative, and it stops when the mouse stops.
var base_aim := Vector3(1, 0, 0)

## The tip on the cue ball: `x` is the distance to the shooter's right and
## `y` the distance above the centre, both in metres.
##
## The tip comes home between the shots: the english is a choice a player
## makes for one stroke, not a state the game keeps for them, and a game that
## carries the english over is one where every shot is crooked for no reason
## the player can see. `_settle` puts it back to the middle.
var contact := Vector2.ZERO

## How far the cue is drawn back now, in metres.
var draw := 0.0

## How far the cue has been drawn back at its furthest, in metres: that is
## the power of the shot.
var pulled := 0.0

## Whether the mouse turns the shot.
##
## The game turns the shot with the mouse, the way a first person game turns
## the view. The tests turn this off: the mouse that happens to be on the desk
## must not be able to change the shot a test is about.
var mouse_turns_the_shot := true

## Where the mouse is.
var mouse_position := Vector2.ZERO

## Whether the mouse has moved yet. Until it has, the aim stays where the
## game put it.
var mouse_moved := false

## Whether the debug panel is up. **D** turns it on and off: the vector of
## the shot while aiming, the state of the balls while the shot runs, and
## the whole report printed for every shot.
var debug := false

## The debug panel.
var debug_panel: Label

## The ball the cue ball is going to hit, by its index in `balls`, and `-1`
## when nothing is on the line. The debug panel reads it.
var aim_hit_ball := -1

## The aim the shot was let off along, kept for the debug panel: the mouse
## still turns the aim while the shot runs, and the panel has to talk about
## the shot that was taken.
var shot_aim := Vector3.ZERO

## The time of the shot being played back, in seconds.
var shot_time := 0.0

## How long the cue has been travelling into the ball, in seconds.
var strike_time := 0.0

## The length of the cue, in metres.
var cue_length := 1.45

## The radius of a ball, in metres.
var ball_radius := 0.028575

## The status line.
var status: Label

## The power bar.
var power: ColorRect

## Where to write the screenshot, if one was asked for.
var screenshot := ""

## How many frames to wait before the picture, so that the game has drawn.
var screenshot_frames := 0


func _ready() -> void:
	core = PoolCore.new()
	var geometry: Dictionary = core.get_table_geometry()
	ball_radius = geometry["ball_radius"]
	var configuration: Dictionary = JSON.parse_string(core.get_table_config())
	cue_length = configuration["cue"]["cue_length"]

	_build_world(geometry)
	_build_balls()
	_build_cue()
	_build_camera(geometry)
	_build_status()
	mouse_position = get_viewport().get_visible_rect().size / 2.0
	_aim_at_the_table()
	_read_the_arguments()


## Builds the table and the light.
func _build_world(geometry: Dictionary) -> void:
	add_child(PoolTable.build(core))

	var light := DirectionalLight3D.new()
	light.rotation_degrees = Vector3(-68, 34, 0)
	light.light_energy = 1.05
	light.shadow_enabled = true
	add_child(light)

	var fill := DirectionalLight3D.new()
	fill.rotation_degrees = Vector3(-32, -140, 0)
	fill.light_energy = 0.32
	add_child(fill)


## Builds the sixteen balls.
func _build_balls() -> void:
	var rack: Dictionary = PoolRack.rack(core)
	var positions: Array = rack["positions"]
	var numbers: Array = rack["numbers"]
	for index in positions.size():
		var ball := PoolBall.new()
		ball.setup(numbers[index], ball_radius)
		add_child(ball)
		ball.place(_to_godot(positions[index]), Vector3.ZERO)
		balls.append(ball)
		live.append(index)
	cue_spot = balls[0].position


## Builds the cue, the aim line, the ghost ball and the tip's mark.
func _build_cue() -> void:
	cue = MeshInstance3D.new()
	var stick := CylinderMesh.new()
	stick.top_radius = 0.0065
	stick.bottom_radius = 0.010
	stick.height = cue_length
	stick.radial_segments = 16
	cue.mesh = stick
	var material := StandardMaterial3D.new()
	material.albedo_color = Color(0.62, 0.45, 0.25)
	material.roughness = 0.45
	cue.material_override = material
	add_child(cue)

	aim_line = MeshInstance3D.new()
	var line := BoxMesh.new()
	line.size = Vector3(0.5, 0.003, 0.003)
	aim_line.mesh = line
	var line_material := StandardMaterial3D.new()
	line_material.albedo_color = Color(0.95, 0.95, 0.95, 0.55)
	line_material.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
	aim_line.material_override = line_material
	add_child(aim_line)

	ghost = MeshInstance3D.new()
	var sphere := SphereMesh.new()
	sphere.radius = ball_radius
	sphere.height = ball_radius * 2.0
	sphere.radial_segments = 32
	sphere.rings = 16
	ghost.mesh = sphere
	var ghost_material := StandardMaterial3D.new()
	ghost_material.albedo_color = Color(0.95, 0.95, 0.95, 0.25)
	ghost_material.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
	ghost.material_override = ghost_material
	add_child(ghost)

	spin_marker = MeshInstance3D.new()
	var mark := SphereMesh.new()
	mark.radius = ball_radius * 0.28
	mark.height = ball_radius * 0.56
	mark.radial_segments = 12
	mark.rings = 8
	spin_marker.mesh = mark
	var mark_material := StandardMaterial3D.new()
	mark_material.albedo_color = Color(0.85, 0.2, 0.2)
	spin_marker.material_override = mark_material
	add_child(spin_marker)


## Builds the camera.
func _build_camera(geometry: Dictionary) -> void:
	var length: float = geometry["length"]
	var width: float = geometry["width"]
	table_middle = Vector3(length / 2.0, 0.0, -width / 2.0)

	# The table camera: it looks down at the table from behind the head rail,
	# and it never moves.
	table_camera = Camera3D.new()
	table_camera.fov = camera_fov
	table_camera.near = 0.01
	table_camera.far = 60.0
	# The camera is not in the tree yet, so it is aimed without `look_at`.
	table_camera.look_at_from_position(
		table_middle + table_camera_offset,
		table_middle + Vector3(0.0, 0.06, 0.05)
	)
	add_child(table_camera)

	# The camera the player looks through. It has to be the current one: the
	# first camera in the tree is the one Godot renders through.
	camera = Camera3D.new()
	camera.fov = camera_fov
	camera.near = 0.01
	camera.far = 60.0
	camera.position = table_camera.position
	camera.basis = table_camera.basis
	add_child(camera)
	camera.make_current()


## Puts the camera where it belongs.
##
## The cue camera rests behind the cue ball, on the line of the shot, and looks
## forward along it: it moves with the aim, so turning the shot turns the view.
func _update_camera() -> void:
	camera.fov = camera_fov
	if camera_mode == 1 and not balls.is_empty():
		var aim := _aim()
		var cue_ball := balls[0]
		var eye := cue_ball.position - aim * cue_camera_back + Vector3.UP * cue_camera_height
		# The view looks along the shot, level.
		camera.position = eye
		camera.look_at(eye + aim * cue_camera_ahead, Vector3.UP)
		return
	# The table: the whole of it, from above and behind - the same view the
	# aim is picked with, so the player can see how the mouse maps.
	camera.position = table_camera.position
	camera.basis = table_camera.basis


## Builds the status line and the power bar.
func _build_status() -> void:
	var layer := CanvasLayer.new()
	add_child(layer)

	status = Label.new()
	status.position = Vector2(16, 12)
	status.add_theme_font_size_override("font_size", 18)
	status.add_theme_color_override("font_color", Color(0.92, 0.92, 0.88))
	layer.add_child(status)

	power = ColorRect.new()
	power.color = Color(0.95, 0.75, 0.2)
	power.position = Vector2(16, 44)
	power.size = Vector2(4, 0)
	layer.add_child(power)

	# Which physics the game is running.
	#
	# The physics is in the extension, and the extension is a library that is
	# loaded when the game starts: a game that is left running keeps the
	# physics it was started with, however new the library on disk is. So the
	# game says which physics it has, and the number changes whenever the
	# physics changes.
	var stamp := Label.new()
	stamp.set_anchors_preset(Control.PRESET_BOTTOM_LEFT)
	stamp.position = Vector2(16, -28)
	stamp.add_theme_font_size_override("font_size", 14)
	stamp.add_theme_color_override("font_color", Color(0.62, 0.62, 0.58))
	stamp.text = "physics " + core.get_model_version()
	layer.add_child(stamp)
	print("the physics is model ", core.get_model_version())

	# The debug panel. **D** turns it on: the vector of the shot while the
	# player aims, the state of the balls while the shot runs, in the same
	# numbers the physics uses.
	debug_panel = Label.new()
	debug_panel.set_anchors_preset(Control.PRESET_TOP_RIGHT)
	debug_panel.position = Vector2(-620, 44)
	debug_panel.add_theme_font_size_override("font_size", 15)
	debug_panel.add_theme_color_override("font_color", Color(0.86, 0.92, 0.80))
	var mono := SystemFont.new()
	mono.font_names = PackedStringArray(["DejaVu Sans Mono", "Monospace", "monospace"])
	debug_panel.add_theme_font_override("font", mono)
	debug_panel.horizontal_alignment = HORIZONTAL_ALIGNMENT_RIGHT
	debug_panel.visible = false
	layer.add_child(debug_panel)


## Reads the command line: `--screenshot PATH` writes a picture of the game
## and leaves.
func _read_the_arguments() -> void:
	var arguments := OS.get_cmdline_user_args()
	for index in arguments.size():
		if arguments[index] == "--screenshot" and index + 1 < arguments.size():
			screenshot = arguments[index + 1]


func _process(delta: float) -> void:
	match phase:
		Phase.AIMING:
			_draw_aim()
		Phase.CHARGING:
			_draw_aim()
			_draw_power()
		Phase.STRIKING:
			strike_time += delta
			_draw_aim()
			if strike_time >= STRIKE_TIME:
				_fire()
		Phase.ROLLING:
			shot_time += delta
			_play_back()
			if shot_time >= shot.get_duration():
				_settle()
	_update_cue()
	_update_aim_visibility()
	_update_camera()
	_update_status()
	_update_debug()
	if screenshot != "" and get_tree().current_scene == self:
		screenshot_frames += 1
		if screenshot_frames > 8:
			_write_the_screenshot()


## Writes the picture and leaves.
func _write_the_screenshot() -> void:
	var path := screenshot
	screenshot = ""
	await RenderingServer.frame_post_draw
	var image := get_viewport().get_texture().get_image()
	var error := image.save_png(path)
	if error != OK:
		printerr("the screenshot failed: ", error)
	else:
		print("the screenshot is at ", path)
	get_tree().quit()


## The core's frame in Godot's.
##
## The core has the height in `z` and Godot has it in `y`, and one of the two
## in-plane axes changes sign so that the frame stays right handed - which is
## what keeps the sense of the spin.
func _to_godot(core: Vector3) -> Vector3:
	return Vector3(core.x, core.z, -core.y)


## Aims at the apex ball of the rack, to begin with.
func _aim_at_the_table() -> void:
	var cue_ball := balls[0]
	var geometry: Dictionary = core.get_table_geometry()
	var apex := _to_godot(Vector3(geometry["length"] / 4.0, geometry["width"] / 2.0, 0.0))
	var direction := apex - cue_ball.position
	direction.y = 0.0
	base_aim = direction.normalized()


## The direction of the shot.
##
## The shot is level. A cue that is tilted down would drive the ball into the
## slate, and the slate is what launches it (§7.1) - so a level shot is not a
## detail here, it is the difference between a break and a jump shot.
func _aim() -> Vector3:
	return base_aim.normalized()


## The direction the cue ball will really take.
##
## The cue points where the player aims, but the ball leaves along the cue
## axis only when the tip is in the middle of the ball: an off-centre tip
## slides on the ball and the ball squirts away from the side that was struck
## (§6.1). The preview shows the line the ball really takes, so the squirt is
## something the player can see and correct for, and not a surprise.
func _shot_direction() -> Vector3:
	var aim := _aim()
	if contact.length_squared() < 1e-12:
		return aim
	var preview: Dictionary = core.preview_cue_strike(1.0, aim, contact)
	var velocity: Vector3 = preview["velocity"]
	if velocity.length_squared() < 1e-12:
		return aim
	return velocity.normalized()


## Draws the aim: the line the cue ball takes, the ball it hits and the mark
## where the tip strikes.
func _draw_aim() -> void:
	var cue_ball := balls[0]
	var aim := _aim()
	var direction := _shot_direction()
	aim_line.position = cue_ball.position + direction * 0.30 + Vector3(0, 0.001, 0)
	aim_line.basis = _basis_along(direction)

	var hit := _first_contact(direction)
	aim_has_hit = hit != Vector3.ZERO
	if aim_has_hit:
		ghost.position = hit

	# The mark where the tip strikes.
	var offset := _right(aim) * contact.x + Vector3.UP * contact.y
	spin_marker.position = cue_ball.position + offset.normalized() * ball_radius


## Shows the aim only while the player is aiming.
##
## The aim is drawn on the cloth, and the camera rides the cue ball: when the
## shot goes, the camera leaves the line and the ghost ball behind, and they
## slide up the screen and off. They have to go with the shot.
func _update_aim_visibility() -> void:
	var aiming := phase == Phase.AIMING or phase == Phase.CHARGING or phase == Phase.STRIKING
	aim_line.visible = aiming
	ghost.visible = aiming and aim_has_hit
	spin_marker.visible = aiming and contact.length_squared() > 1e-12


## A basis that puts the +x axis along the direction.
func _basis_along(direction: Vector3) -> Basis:
	var forward := direction.normalized()
	var reference := Vector3.UP
	if absf(forward.dot(reference)) > 0.99:
		reference = Vector3.FORWARD
	var side := reference.cross(forward).normalized()
	var up := side.cross(forward).normalized()
	return Basis(forward, up, side)


## A basis that puts the +y axis along the direction: the cue is a cylinder,
## and a cylinder stands along +y.
func _basis_along_y(direction: Vector3) -> Basis:
	var forward := direction.normalized()
	var reference := Vector3.FORWARD
	if absf(forward.dot(reference)) > 0.99:
		reference = Vector3.RIGHT
	var right := reference.cross(forward).normalized()
	var back := right.cross(forward).normalized()
	return Basis(right, forward, back)


## The shooter's right, for a shot along the direction.
func _right(direction: Vector3) -> Vector3:
	return direction.cross(Vector3.UP).normalized()


## The first ball the cue ball meets, and where its centre would be.
##
## The positions are the ones the balls have, not a copy of them: the preview
## has to be about the table the balls are on.
##
## The ball it found is left in `aim_hit_ball`, so the debug panel can name
## it without hunting for it a second time.
func _first_contact(aim: Vector3) -> Vector3:
	var cue_ball := balls[0]
	var best := INF
	var found := Vector3.ZERO
	aim_hit_ball = -1
	for index in live:
		if index == 0:
			continue
		var ball := balls[index]
		var to_ball := ball.position - cue_ball.position
		var along := to_ball.dot(aim)
		if along <= 0.0 or along >= best:
			continue
		var across := (to_ball - aim * along).length()
		if across > 2.0 * ball_radius:
			continue
		best = along
		aim_hit_ball = index
		var back := sqrt(maxf(0.0, 4.0 * ball_radius * ball_radius - across * across))
		found = cue_ball.position + aim * (along - back)
	return found


## A vector, to the millimetre and the four decimals: `(-1.1294, 0.0000, 0.0000)`.
func _vector(v: Vector3) -> String:
	return "(%.4f, %.4f, %.4f)" % [v.x, v.y, v.z]

## Draws the power bar.
func _draw_power() -> void:
	power.size = Vector2(4.0 + 200.0 * pulled / MAX_PULL, 14)


## Puts the cue where it belongs: behind the cue ball, along the aim, drawn
## back by as much as the mouse has pulled it.
func _update_cue() -> void:
	var cue_ball := balls[0]
	var aim := _aim()
	var tip := _tip(cue_ball.position, aim)
	var travel := draw
	if phase == Phase.STRIKING:
		travel = draw * (1.0 - clampf(strike_time / STRIKE_TIME, 0.0, 1.0))
	cue.visible = phase != Phase.ROLLING
	cue.position = tip - aim * (cue_length / 2.0 + travel)
	cue.basis = _basis_along_y(aim)


## Where the tip of the cue touches the cue ball.
##
## The tip strikes the ball off centre, so the cue's axis misses the centre
## of the ball by the impact parameter.
func _tip(centre: Vector3, aim: Vector3) -> Vector3:
	var offset := _right(aim) * contact.x + Vector3.UP * contact.y
	if offset.length() < 0.000001:
		return centre - aim * ball_radius
	return centre + offset.normalized() * ball_radius


## Fires the shot at once, without the mouse.
##
## This is the way the play test starts a shot: the mouse is not there.
func fire_now(speed: float) -> void:
	pulled = MAX_PULL * clampf(speed / MAX_CUE_SPEED, 0.0, 1.0)
	phase = Phase.STRIKING
	strike_time = 0.0


## Fires the shot.
func _fire() -> void:
	var aim := _aim()
	var positions := PackedVector3Array()
	var numbers := PackedInt32Array()
	for index in live:
		positions.append(balls[index].position)
		numbers.append(balls[index].number)

	# The power of the shot is how far the cue was drawn back.
	var speed := maxf(pulled / MAX_PULL * MAX_CUE_SPEED, 0.15)
	shot = core.simulate(speed, aim, contact, positions, numbers)
	shot_aim = aim
	aim_line.visible = false
	phase = Phase.ROLLING
	shot_time = 0.0
	if shot.get_error() != "":
		status.text = "the shot faulted: " + shot.get_error()
	else:
		_print_the_shot_report(aim, speed)


## Plays the path back.
func _play_back() -> void:
	var positions := shot.sample(shot_time)
	var rotations := shot.sample_rotation(shot_time)
	for index in live.size():
		if index < positions.size():
			balls[live[index]].place(positions[index], rotations[index])


## The shot is over.
##
## The balls that left the table are out of the shot, and the cue ball goes
## back on the table if it was pocketed: there is no ball-in-hand rule yet,
## and a game you cannot shoot in again is no use.
func _settle() -> void:
	phase = Phase.AIMING
	pulled = 0.0
	draw = 0.0
	# The tip comes home: the next shot is a centre-ball stroke unless the
	# player asks for the english again.
	contact = Vector2.ZERO
	var still: Array[int] = []
	off_table = []
	pocketed = []
	for index in live.size():
		var state := shot.get_ball_state(index)
		if state.get("pocketed", false):
			# The ball went into a pocket: it is out of play, but it is not
			# a ball that left the table.
			balls[live[index]].visible = false
			pocketed.append(live[index])
		elif state.get("off_table", false):
			balls[live[index]].visible = false
			off_table.append(live[index])
		else:
			still.append(live[index])
	live = still
	if not live.has(0):
		balls[0].place(cue_spot, Vector3.ZERO)
		balls[0].visible = true
		live.insert(0, 0)
	_print_the_end_report()


## The status line.
func _update_status() -> void:
	match phase:
		Phase.AIMING:
			status.text = _aiming_status()
		Phase.CHARGING:
			status.text = "draw  -  push the mouse forward to shoot"
		Phase.STRIKING:
			status.text = "..."
		Phase.ROLLING:
			status.text = "the balls are running"


## The aiming line of the status: it says where the tip is and where the ball
## will leave, so the english is something the player reads, not something
## they have to spot on the ball.
##
## The tip is given in millimetres, the squirt in degrees, and the side by
## the words a player uses: the tip sits to the shooter's left or right, and
## the ball leaves the other way - the way of it comes from the preview, the
## only source that cannot be wrong on one side of it.
func _aiming_status() -> String:
	var text := "aim  -  the mouse turns the shot; S draws the cue back, E for the english, B for a finer aim, D for the debug panel"
	if contact.length_squared() < 1e-12:
		return text
	var aim := _aim()
	var preview: Dictionary = core.preview_cue_strike(1.0, aim, contact)
	var velocity: Vector3 = preview["velocity"]
	var across := velocity.dot(_right(aim))
	var leave := roundi(absf(atan2(across, velocity.dot(aim))) * 57.29578 * 100.0) / 100.0
	if absf(contact.x) < 1e-6:
		# The tip is over the centre of the ball: follow or draw, and a shot
		# that leaves straight.
		var height := roundi(contact.y * 1000.0)
		return text + "   (tip %d mm %s, the ball leaves straight)" % [
			absi(height), "above" if contact.y > 0.0 else "below"]
	var tip_side := "right" if contact.x > 0.0 else "left"
	var leave_side := "right" if across > 0.0 else "left"
	var tip := roundi(contact.x * 1000.0)
	return text + "   (tip %d mm %s, the ball leaves %s by %s deg)" % [absi(tip), tip_side, leave_side, leave]


## The debug panel. **D** raises it: while the player aims it names the shot,
## while the shot runs it names the cue ball. The numbers it gives are the
## ones the physics has, not the ones the picture draws, so the panel and a
## test read the same line.
func _update_debug() -> void:
	debug_panel.visible = debug
	if not debug:
		return
	if phase == Phase.ROLLING:
		_debug_the_run()
	else:
		_debug_the_shot()


## The shot as it is being aimed: the line the cue is on, the line the ball
## really leaves on, the tip, the ball the ghost waits on and the strength.
## The two shot lines differ by the squirt, and it is put down the same way
## the status puts it: the side of the tip, the other side the ball leaves.
func _debug_the_shot() -> void:
	var aim := _aim()
	var direction := _shot_direction()
	var lines: Array[String] = []
	lines.append("the aim    " + _vector(aim))
	var shot_line := "the shot   " + _vector(direction)
	var across := direction.dot(_right(aim))
	if absf(across) > 1e-9:
		var deg := rad_to_deg(atan2(absf(across), direction.dot(aim)))
		shot_line += "   %.2f deg %s" % [deg, "left" if across < 0.0 else "right"]
	lines.append(shot_line)
	lines.append("the tip    " + _tip_words())
	if aim_hit_ball >= 0:
		var cue_ball := balls[0]
		lines.append("the ghost  ball %d in %.2f m" % [balls[aim_hit_ball].number, ghost.position.distance_to(cue_ball.position)])
	else:
		lines.append("the ghost  nothing on the line")
	var speed := maxf(pulled / MAX_PULL * MAX_CUE_SPEED, 0.15)
	lines.append("the stroke %.2f m/s (a full draw is %.2f)" % [speed, MAX_CUE_SPEED])
	debug_panel.text = "\n".join(lines)


## The run of the shot: where the cue ball runs at this very moment, the
## spin it carries and the events resolved so far.
func _debug_the_run() -> void:
	var lines: Array[String] = []
	lines.append("the run    %.3f / %.3f s" % [shot_time, shot.get_duration()])
	var velocity := _trace_velocity(shot_time, 0)
	lines.append("the cue    %.2f m/s   %s" % [velocity.length(), _vector(velocity)])
	var omega: Vector3 = shot.sample_spin(shot_time)[0]
	lines.append("the spin   " + _spin_words(omega, shot_aim))
	lines.append("the balls  " + _events_words(shot_time))
	debug_panel.text = "\n".join(lines)


## How fast a ball runs at time `t`, read off the path: two samples a hair
## of a second apart, divided by the time between them.
func _trace_velocity(t: float, index: int) -> Vector3:
	var step := minf(t + 0.002, shot.get_duration()) - t
	if step < 1e-4:
		return Vector3.ZERO
	var back: Vector3 = shot.sample(t)[index]
	var now: Vector3 = shot.sample(t + step)[index]
	return (now - back) / step


## The spin in the words a player uses: follow or draw along the line of the
## shot, and the english across it. The rate is how fast the ball turns, in
## radians a second, and it comes off the path.
func _spin_words(omega: Vector3, aim: Vector3) -> String:
	var rate := omega.length()
	if rate < 1.0:
		# A rad a second is a sixteenth of a turn: under that the ball is
		# only sliding, and the words for a stroke that imparts no spin are
		# the truth.
		return "no spin: the stroke was a stun"
	var follow := -omega.dot(_right(aim)) / rate
	var side := omega.dot(Vector3.UP) / rate
	var words: Array[String] = []
	if absf(follow) > 0.12:
		words.append("follow" if follow > 0.0 else "draw")
	if absf(side) > 0.12:
		words.append("right english" if side > 0.0 else "left english")
	if words.is_empty():
		return "%.0f rad/s about %s" % [rate, _vector(omega.normalized())]
	return "%.0f rad/s - %s" % [rate, " and ".join(words)]


## The events resolved so far: how many, and the last of them.
func _events_words(t: float) -> String:
	var events: Array = shot.get_events()
	var so_far := 0
	var last: Dictionary = {}
	for event in events:
		if float(event["time"]) <= t:
			so_far += 1
			last = event
	if so_far == 0:
		return "nothing touched yet"
	return "%d so far, the last was %s at %.2f s" % [so_far, String(last["kind"]), float(last["time"])]


## The tip in the player's words: the millimetres it sits off the centre, to
## the shooter's right or left and above or below.
func _tip_words() -> String:
	if contact.length_squared() < 1e-12:
		return "over the centre"
	var across := roundi(absf(contact.x) * 1000.0)
	var height := roundi(absf(contact.y) * 1000.0)
	var words: Array[String] = []
	if across > 0:
		words.append("%d mm %s" % [across, "right" if contact.x > 0.0 else "left"])
	if height > 0:
		words.append("%d mm %s" % [height, "above" if contact.y > 0.0 else "below"])
	if words.is_empty():
		return "over the centre"
	return " and ".join(words)


## The whole shot, printed for the log: the tip, the stroke, the line the
## ball really takes, the spin and the length of the run. The panel shows
## the same numbers while the game runs; the log keeps them after it.
func _print_the_shot_report(aim: Vector3, speed: float) -> void:
	print("the shot went in:")
	print("  the tip: ", _tip_words())
	print("  the stroke: %.2f m/s along %s" % [speed, _vector(aim)])
	var preview: Dictionary = core.preview_cue_strike(speed, aim, contact)
	var velocity: Vector3 = preview["velocity"]
	var across := velocity.dot(_right(aim))
	var deg := rad_to_deg(atan2(absf(across), velocity.dot(aim)))
	if deg < 0.005:
		# The ball leaves on the line of the cue: there is no side to name,
		# and naming one would be a lie.
		print("  the ball leaves straight along the aim")
	else:
		print("  the ball leaves the aim by %.2f deg, %s" % [deg, "left" if across < 0.0 else "right"])
	print("  it leaves along ", _vector(velocity.normalized()))
	var omega: Vector3 = shot.sample_spin(0.001)[0]
	print("  the spin: ", _spin_words(omega, aim))
	print("  the run: %.3f s, %d events, hash %s" % [shot.get_duration(), shot.get_events().size(), shot.get_hash()])


## The end of the shot, printed for the log: what went down, what left the
## table and what the rules made of the shot.
func _print_the_end_report() -> void:
	var final_state: Dictionary = shot.get_final_state()
	var down := _numbers_words(pocketed)
	var off := _numbers_words(off_table)
	print("the shot ended after %.3f s: %d ball(s) down [%s], %d off the table [%s], the rules say: foul %s, miscue %s" % [
		float(final_state["elapsed_time"]), pocketed.size(), down,
		off_table.size(), off,
		"yes" if final_state["foul"] else "no",
		"yes" if final_state["miscue"] else "no"])


## The balls of a list, named by their numbers: `5, 12`.
func _numbers_words(list: Array) -> String:
	var names: Array[String] = []
	for index in list:
		names.append(str(balls[index].number))
	return ", ".join(names)


func _unhandled_input(event: InputEvent) -> void:
	if event is InputEventMouseMotion:
		_mouse_motion(event)
	elif event is InputEventKey:
		_key(event)


## The mouse: it turns the shot, or it draws the cue, or it moves the tip.
func _mouse_motion(event: InputEventMouseMotion) -> void:
	mouse_moved = true
	mouse_position = event.position

	if not mouse_turns_the_shot:
		return

	if Input.is_key_pressed(KEY_E):
		# The tip moves on the face of the ball, and the aim is left alone.
		contact.x += event.relative.x * SPIN_SCALE
		contact.y -= event.relative.y * SPIN_SCALE
		var limit: float = core.get_miscue_limit()
		contact.x = clampf(contact.x, -limit, limit)
		contact.y = clampf(contact.y, -limit, limit)
		return

	if phase == Phase.CHARGING:
		# The cue is drawn back by dragging the mouse towards the player - down
		# the screen - and the shot is on when the cue comes back to the ball,
		# so the key never has to be let go of.
		var was_drawn := pulled > 0.0
		draw = clampf(draw + event.relative.y * DRAW_SPEED, 0.0, MAX_PULL)
		if was_drawn and event.relative.y < 0.0 and draw <= 0.0:
			# The cue is home: the stroke is over.
			phase = Phase.STRIKING
			strike_time = 0.0
			return
		pulled = maxf(pulled, draw)
		return

	# The mouse turns the shot, the way it turns the view in a first person
	# game: relative, and it stops when the mouse stops. **B** turns it finer.
	var fine := 1.0
	if Input.is_key_pressed(KEY_B):
		fine = FINE_SCALE
	base_aim = base_aim.rotated(Vector3.UP, -event.relative.x * LOOK_SPEED * fine)


## The keys.
func _key(event: InputEventKey) -> void:
	if event.keycode == KEY_S:
		if event.pressed and not event.echo and phase == Phase.AIMING:
			# The cue is drawn back. The shot is on when the cue comes home,
			# so letting go of the key only ever cancels.
			phase = Phase.CHARGING
			draw = 0.0
			pulled = 0.0
		elif not event.pressed and phase == Phase.CHARGING:
			# The key is let go of: back to the default, the draw is
			# forgotten, and no shot is on.
			phase = Phase.AIMING
			draw = 0.0
			pulled = 0.0
	elif event.keycode == KEY_ESCAPE and phase == Phase.CHARGING:
		phase = Phase.AIMING
		draw = 0.0
		pulled = 0.0
	elif event.keycode == KEY_C:
		camera_mode = 1 - camera_mode
	elif event.keycode == KEY_D and event.pressed and not event.echo:
		# The debug panel. The report is printed for every shot whether the
		# panel is up or not, so a log is readable even when the picture
		# is not.
		debug = not debug
