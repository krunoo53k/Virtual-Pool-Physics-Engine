extends Node3D

## A play test: the game, a break, and the balls where the core put them.
##
## The mouse is not there, so the shot is fired through the hook the game
## offers; everything after that is the game itself - the core resolves the
## shot, and the game plays the path back.
##
##     godot --headless --path godot-project scenes/play_test.tscn
##
## The exit code is 0 when the break behaved, 1 when it did not.

## The game under test.
var game: Node3D

## The positions of the balls before the break.
var before: Array[Vector3] = []

## Where to write the picture, if one was asked for.
var screenshot := ""

## Whether the picture has been taken.
var taken := false


func _ready() -> void:
	game = preload("res://scripts/main.gd").new()
	add_child(game)
	await get_tree().process_frame
	await get_tree().process_frame

	for ball in game.balls:
		before.append(ball.position)

	# The break: straight into the rack, at full power.
	game.fire_now(9.0)

	# The picture, if one was asked for, is taken while the balls are running.
	for argument in OS.get_cmdline_user_args():
		if argument == "--screenshot" and OS.get_cmdline_user_args().size() > 0:
			var arguments := OS.get_cmdline_user_args()
			var index := arguments.find("--screenshot")
			if index >= 0 and index + 1 < arguments.size():
				screenshot = arguments[index + 1]

	# The shot takes as long as the core says it does.
	var limit := 0
	while game.phase == game.Phase.STRIKING or game.phase == game.Phase.ROLLING:
		if limit > 20000:
			break
		limit += 1
		await get_tree().process_frame
		if screenshot != "" and not taken and game.phase == game.Phase.ROLLING:
			if game.shot_time > 1.1:
				taken = true
				await RenderingServer.frame_post_draw
				var image := get_viewport().get_texture().get_image()
				image.save_png(screenshot)
				print("the screenshot is at ", screenshot)

	_check()


## Checks that the break happened.
func _check() -> void:
	var moved := 0
	for index in game.balls.size():
		if game.balls[index].position.distance_to(before[index]) > 0.01:
			moved += 1
	print("the break moved ", moved, " balls")
	print("the cue ball is at ", game.balls[0].position)
	print("the shot took ", game.shot.get_duration(), " s")

	var ok := moved >= 10
	if ok:
		print("the break behaved")
	else:
		printerr("the break did not behave: only ", moved, " balls moved")
	get_tree().quit(0 if ok else 1)
