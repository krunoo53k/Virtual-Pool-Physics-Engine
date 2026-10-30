class_name PoolBall
extends Node3D

## A ball on the table.
##
## The ball is a node so that the game can move it, and the rotation of the
## mesh is kept apart from the position: the core gives the rotation as a
## rotation vector - the axis of the rotation times the angle - and the mesh
## turns about that axis.

const PoolTextures := preload("res://scripts/textures.gd")

## The number of the ball: 0 is the cue ball.
var number: int = 0

## The mesh of the ball.
var mesh: MeshInstance3D

## The orientation the ball carries into the next shot, as a rotation vector:
## its direction is the axis of the orientation, its length the angle in radians.
##
## The hand-off of §11.2 on the game's side. The core counts a ball's rotation
## from the orientation the ball is given at the start of the shot, so the game
## hands back the orientation it last played the ball turned to: a ball still
## lying where the last shot left it keeps its phase, and its spin does not
## reset when the next shot is fired. Only a ball that was put back on the table
## carries `Vector3.ZERO`, and only that one starts over. The number came from
## the core, at the end of a path: nothing here integrates a rotation.
var carried_rotation := Vector3.ZERO

## Builds the ball.
func setup(ball_number: int, radius: float) -> void:
	number = ball_number
	mesh = MeshInstance3D.new()
	var sphere := SphereMesh.new()
	sphere.radius = radius
	sphere.height = radius * 2.0
	sphere.radial_segments = 48
	sphere.rings = 24
	mesh.mesh = sphere
	var material := StandardMaterial3D.new()
	material.albedo_texture = PoolTextures.ball(number)
	material.roughness = 0.22
	material.metallic = 0.0
	mesh.material_override = material
	add_child(mesh)

## Puts the ball where the core says it is, and turns it to the orientation the
## core says it has.
##
## The position is in the core's frame, in metres, and `rotation_vector` is a
## rotation vector: its direction is the axis of the orientation, its length the
## angle in radians.
##
## The rotation is an *orientation* and not a step: the core counts a ball's
## rotation from the orientation the ball is given at the start of the shot, so
## the number it sends back already includes whatever the shot before this one
## left the ball turned by - the hand-off of §11.2. Nothing here integrates a
## rotation, so the renderer stays passive and has no interpolation error to
## have.
	# A rotation vector is the axis of the orientation times its angle in radians,
	# so Godot's `Basis` constructor wants the two apart: the axis, which it asks
	# to be a unit one, and the angle. The floor keeps a vector too short to have
	# a direction out of `normalized`, and a turn that short is one the eye cannot
	# see.
	# With the carry in play a ball can be standing in the phase an earlier shot
	# left it in, so an orientation of no turn at all is written down rather than
	# left standing: the identity orientation, which is what a ball that was put
	# back on the table has.
##
## The orientation is remembered as `carried_rotation`, which is what the game
## hands back to the core at the next shot. A placement at `Vector3.ZERO` is the
## identity orientation, and is what a ball that was put back on the table
## carries: only that one starts over.
func place(ball_position: Vector3, rotation_vector: Vector3) -> void:
	position = ball_position
	var angle := rotation_vector.length()
	if angle > 0.000001:
		# The number is an orientation counted by the core from the identity
		# orientation of the ball, and not a step for the game to add: the mesh is
		# written to it rather than turned on, so the same number places the ball in
		# the same orientation, and nothing here accumulates or drifts. The shot
		# before this one is already counted in the number, which is the hand-off
		# of §11.2, so a ball that is fired or struck again does not jump.
		# The frame the extension maps is a proper rotation, so it carries an axis
		# and its angle across unchanged in meaning: the sense of a spin survives
		# the trip, and the ball turns the way the core said it was turning.
		# The number on the ball is on the mesh, so a turned ball shows a turned
		# number: the turn is the roll.
		# The orientation came from the core, at the end of a path: the physics is
		# not here, and nothing here integrates, so the renderer has no
		# interpolation error to have.
		# The node itself stays unturned so that the ball can be moved about the
		# table without tilting; the mesh of the ball carries the orientation.
		mesh.basis = Basis(rotation_vector.normalized(), angle)
	else:
	# `Basis()` is the identity orientation.
	# The ball is put down straight: a respot, or a fresh build.
	# A turn too short to have a direction is not a turn to see.
		mesh.basis = Basis()
