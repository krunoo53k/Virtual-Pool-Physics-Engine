class_name PoolTextures
extends RefCounted

## The textures of the balls, made in code.
##
## There is no art pipeline yet, so the balls are drawn: the texture of a
## sphere in Godot is a map of the sphere onto a rectangle, so a stripe is a
## band across the picture and a spot is a circle on it.
##
## Every ball gets a band and a spot. The band shows the roll - a ball that
## rolls turns about its side, and the band tips with it - and the spot shows
## the spin about the vertical, which the band alone cannot show, because a
## band around the vertical axis looks the same however far it turns.

## The colours of the object balls: 1 to 7 are solid, 8 is black, 9 to 15 are
## the stripes.
const COLOURS := {
	1: Color(0.95, 0.78, 0.14),
	2: Color(0.16, 0.29, 0.66),
	3: Color(0.78, 0.18, 0.18),
	4: Color(0.35, 0.20, 0.52),
	5: Color(0.93, 0.47, 0.13),
	6: Color(0.16, 0.45, 0.24),
	7: Color(0.55, 0.14, 0.14),
	8: Color(0.09, 0.09, 0.10),
	9: Color(0.95, 0.78, 0.14),
	10: Color(0.16, 0.29, 0.66),
	11: Color(0.78, 0.18, 0.18),
	12: Color(0.35, 0.20, 0.52),
	13: Color(0.93, 0.47, 0.13),
	14: Color(0.16, 0.45, 0.24),
	15: Color(0.55, 0.14, 0.14),
}

## The colour of the cue ball: plain white, with nothing on it.
const CUE_COLOUR := Color(1.0, 1.0, 1.0)

## The texture of a ball.
##
## `number` is 0 for the cue ball.
static func ball(number: int, size: int = 128) -> ImageTexture:
	var image := Image.create(size, size, false, Image.FORMAT_RGB8)
	var base := CUE_COLOUR
	var band := CUE_COLOUR
	if number != 0:
		base = COLOURS.get(number, CUE_COLOUR)
		# The solid balls are their colour all over; the stripes are white with
		# a band of colour.
		if number >= 9:
			base = CUE_COLOUR
			band = COLOURS.get(number, CUE_COLOUR)
		else:
			band = CUE_COLOUR
	for y in size:
		var v := float(y) / float(size - 1)
		for x in size:
			var u := float(x) / float(size - 1)
			var colour := base
			# The band around the waist.
			if v > 0.44 and v < 0.56:
				colour = band
			# The spot, off to one side so that the spin about the vertical
			# shows: a band on its own cannot.
			if _in_spot(u, v):
				colour = band
			image.set_pixel(x, y, colour)
	return ImageTexture.create_from_image(image)

## True when the point of the picture is inside the spot.
##
## The picture is a map of the sphere, so a circle on the ball is an ellipse
## on the picture: it is twice as tall as it is wide.
static func _in_spot(u: float, v: float) -> bool:
	var du := u - 0.28
	var dv := v - 0.30
	return du * du * 4.0 + dv * dv < 0.0036
