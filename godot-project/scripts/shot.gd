extends Node3D

## A temporary script: the game, with the camera set by hand, and a picture of
## it. The arguments are the camera's back, height, ahead and the file to
## write.
##
##     godot --path godot-project scenes/shot.tscn -- back,height,ahead,file.png

func _ready() -> void:
	var game := preload("res://scripts/main.gd").new()
	add_child(game)
	await get_tree().process_frame
	await get_tree().process_frame

	var arguments := OS.get_cmdline_user_args()
	var settings := "0.62,0.155,2.4"
	var path := "/tmp/opencode/shot.png"
	if arguments.size() > 0:
		settings = arguments[0]
	if arguments.size() > 1:
		path = arguments[1]

	var parts := settings.split(",")
	game.camera_mode = 1
	game.cue_camera_back = float(parts[0])
	game.cue_camera_height = float(parts[1])
	game.cue_camera_ahead = float(parts[2])
	if parts.size() > 3:
		game.camera_fov = float(parts[3])
	# A turn: the mouse moves, and the shot and the camera follow it.
	if parts.size() > 4:
		var turn := float(parts[4])
		var motion := InputEventMouseMotion.new()
		motion.position = Vector2(640 + turn, 360)
		motion.relative = Vector2(turn, 0)
		Input.parse_input_event(motion)

	# Let the camera settle on the aim.
	for step in 6:
		await get_tree().process_frame
	await RenderingServer.frame_post_draw
	var image := get_viewport().get_texture().get_image()
	image.save_png(path)
	print("wrote ", path, " with back=", parts[0], " height=", parts[1], " ahead=", parts[2])
	get_tree().quit()
