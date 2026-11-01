class_name FaceCameraController
extends Camera3D

@export var planet_path: NodePath
@export var player_path: NodePath
@export var transition_speed: float = 5.0
@export var camera_distance: float = 24.0
@export var camera_tilt_deg: float = 30.0

var planet: DodecahedronPlanet
var player: CharacterBody3D

var previous_face_index: int = -1
var current_face_index: int = -1
var target_transform: Transform3D

# Normalized basis vectors on the face plane: screen right (+X) and screen up (+Y)
var face_screen_right: Vector3 = Vector3.RIGHT
var face_screen_up: Vector3 = Vector3.UP

func _ready() -> void:
	if planet_path:
		planet = get_node(planet_path) as DodecahedronPlanet
	if player_path:
		player = get_node(player_path) as CharacterBody3D

func _physics_process(delta: float) -> void:
	if not planet or not player:
		return

	# Detect closest face based on player's current position
	var face_idx = planet.get_closest_face_index(player.global_position)
	if face_idx != current_face_index:
		previous_face_index = current_face_index
		current_face_index = face_idx

		# Orient camera so the crossed edge between previous_face_index and current_face_index is bottom of viewport
		target_transform = planet.get_face_camera_transform_from_transition(previous_face_index, current_face_index, camera_distance, camera_tilt_deg)
		if global_transform == Transform3D.IDENTITY:
			global_transform = target_transform

		# Update cardinal axes for the active face (projected onto face normal)
		var face_normal = (planet.global_transform.basis * planet.face_normals[current_face_index]).normalized()
		var cam_r = target_transform.basis.x.normalized()
		var cam_u = target_transform.basis.y.normalized()

		# Ensure right and up are on the face plane
		face_screen_right = (cam_r - face_normal * cam_r.dot(face_normal)).normalized()
		face_screen_up = (cam_u - face_normal * cam_u.dot(face_normal)).normalized()

	# Smoothly interpolate position and orientation to the active face's camera frame
	global_position = global_position.lerp(target_transform.origin, clamp(transition_speed * delta, 0.0, 1.0))

	var cur_q = global_transform.basis.get_rotation_quaternion()
	var target_q = target_transform.basis.get_rotation_quaternion()
	var new_q = cur_q.slerp(target_q, clamp(transition_speed * delta, 0.0, 1.0)).normalized()
	global_transform.basis = Basis(new_q)
