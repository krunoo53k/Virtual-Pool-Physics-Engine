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

## The cue stick.
var cue: MeshInstance3D

## The line the cue ball takes.
var aim_line: MeshInstance3D

## The ball the cue ball is about to hit.
var ghost: MeshInstance3D

## The mark on the cue ball where the tip strikes.
var spin_marker: MeshInstance3D

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
var contact := Vector2.ZERO

## How far the cue is drawn back now, in metres.
var draw := 0.0

## How far the cue has been drawn back at its furthest, in metres: that is
## the power of the shot.
var pulled := 0.0

## Where the mouse is.
var mouse_position := Vector2.ZERO

## Whether the mouse has moved yet. Until it has, the aim stays where the
## game put it.
var mouse_moved := false

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
	_update_camera()
	_update_status()
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


## Draws the aim: the line the cue ball takes, the ball it hits and the mark
## where the tip strikes.
func _draw_aim() -> void:
	var cue_ball := balls[0]
	var aim := _aim()
	aim_line.visible = phase != Phase.ROLLING
	aim_line.position = cue_ball.position + aim * 0.30 + Vector3(0, 0.001, 0)
	aim_line.basis = _basis_along(aim)

	var hit := _first_contact(aim)
	ghost.visible = hit != Vector3.ZERO
	if ghost.visible:
		ghost.position = hit

	# The mark where the tip strikes.
	var offset := _right(aim) * contact.x + Vector3.UP * contact.y
	spin_marker.visible = offset.length() > 0.0001
	if spin_marker.visible:
		spin_marker.position = cue_ball.position + offset.normalized() * ball_radius


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
func _first_contact(aim: Vector3) -> Vector3:
	var cue_ball := balls[0]
	var best := INF
	var found := Vector3.ZERO
	for index in range(1, balls.size()):
		var ball := balls[index]
		var to_ball := ball.position - cue_ball.position
		var along := to_ball.dot(aim)
		if along <= 0.0 or along >= best:
			continue
		var across := (to_ball - aim * along).length()
		if across > 2.0 * ball_radius:
			continue
		best = along
		var back := sqrt(maxf(0.0, 4.0 * ball_radius * ball_radius - across * across))
		found = cue_ball.position + aim * (along - back)
	return found


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
	for ball in balls:
		positions.append(ball.position)
		numbers.append(ball.number)

	# The power of the shot is how far the cue was drawn back.
	var speed := maxf(pulled / MAX_PULL * MAX_CUE_SPEED, 0.15)
	shot = core.simulate(speed, aim, contact, positions, numbers)
	aim_line.visible = false
	phase = Phase.ROLLING
	shot_time = 0.0
	if shot.get_error() != "":
		status.text = "the shot faulted: " + shot.get_error()


## Plays the path back.
func _play_back() -> void:
	var positions := shot.sample(shot_time)
	var rotations := shot.sample_rotation(shot_time)
	for index in balls.size():
		if index < positions.size():
			balls[index].place(positions[index], rotations[index])


## The shot is over.
func _settle() -> void:
	phase = Phase.AIMING
	pulled = 0.0
	draw = 0.0
	for index in balls.size():
		var state := shot.get_ball_state(index)
		if state.get("pocketed", false) or state.get("off_table", false):
			balls[index].visible = false


## The status line.
func _update_status() -> void:
	match phase:
		Phase.AIMING:
			status.text = "aim  -  the mouse turns the shot; S draws the cue back, E for the english, B for a finer aim"
		Phase.CHARGING:
			status.text = "draw  -  push the mouse forward to shoot"
		Phase.STRIKING:
			status.text = "..."
		Phase.ROLLING:
			status.text = "the balls are running"


func _unhandled_input(event: InputEvent) -> void:
	if event is InputEventMouseMotion:
		_mouse_motion(event)
	elif event is InputEventKey:
		_key(event)


## The mouse: it turns the shot, or it draws the cue, or it moves the tip.
func _mouse_motion(event: InputEventMouseMotion) -> void:
	mouse_moved = true
	mouse_position = event.position

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
