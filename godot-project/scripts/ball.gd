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

## Puts the ball where the core says it is.
##
## The position is in the core's frame, in metres, and the rotation is the
## rotation vector: the direction is the axis and the length is the angle.
func place(ball_position: Vector3, rotation_vector: Vector3) -> void:
	position = ball_position
	var angle := rotation_vector.length()
	if angle > 0.000001:
		mesh.basis = Basis(rotation_vector.normalized(), angle)
