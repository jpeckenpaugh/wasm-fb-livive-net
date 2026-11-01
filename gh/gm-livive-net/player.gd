class_name PlanetPlayer
extends CharacterBody3D

@export var move_speed: float = 8.0
@export var acceleration: float = 20.0
@export var jump_velocity: float = 8.0
@export var gravity_strength: float = 25.0
@export var alignment_speed: float = 12.0
@export var mouse_sensitivity: float = 0.003

@export var planet_path: NodePath
var planet: Node3D

@onready var cam_yaw: Node3D = get_node_or_null("CamYaw")
@onready var cam_pitch: Node3D = get_node_or_null("CamYaw/CamPitch")
@onready var camera: Camera3D = get_node_or_null("CamYaw/CamPitch/SpringArm3D/Camera3D")
@onready var ground_ray: RayCast3D = $GroundRay

var current_up: Vector3 = Vector3.UP
var pitch_angle: float = 0.0

func _ready() -> void:
	if planet_path != null and not planet_path.is_empty():
		planet = get_node(planet_path)
	current_up = global_transform.basis.y.normalized()

func _unhandled_input(event: InputEvent) -> void:
	if cam_yaw != null and event is InputEventMouseMotion and Input.get_mouse_mode() == Input.MOUSE_MODE_CAPTURED:
		cam_yaw.rotate_y(-event.relative.x * mouse_sensitivity)
		pitch_angle = clamp(pitch_angle - event.relative.y * mouse_sensitivity, deg_to_rad(-70.0), deg_to_rad(60.0))
		cam_pitch.rotation.x = pitch_angle

@export var face_camera_path: NodePath
var face_camera: FaceCameraController

@export var rotation_speed: float = 15.0

func _physics_process(delta: float) -> void:
	if not face_camera and face_camera_path:
		face_camera = get_node_or_null(face_camera_path) as FaceCameraController

	var planet_center = planet.global_position if planet else Vector3.ZERO
	var to_center = planet_center - global_position
	var spherical_up = -to_center.normalized()

	# 1. Determine target Up normal
	var target_up = spherical_up
	if is_on_floor():
		var floor_n = get_floor_normal()
		if floor_n.length_squared() > 0.01:
			target_up = floor_n
	elif ground_ray.is_colliding():
		var ray_n = ground_ray.get_collision_normal()
		if ray_n.length_squared() > 0.01:
			target_up = ray_n

	# 2. Smoothly align character basis up-vector to target_up
	_align_to_normal(target_up, delta)

	# 3. Apply gravity toward planet center
	var player_up = global_transform.basis.y
	var gravity_dir = -player_up
	if not is_on_floor():
		velocity += gravity_dir * gravity_strength * delta

	# 4. Handle Jump
	if Input.is_action_just_pressed("jump") and is_on_floor():
		velocity += player_up * jump_velocity

	# 5. Determine Cardinal Movement Directions (aligned with active face viewport)
	# "Up" moves from the bottom crossed edge into the face (+face_screen_up)
	# "Down" moves toward the bottom crossed edge (-face_screen_up)
	# "Left" / "Right" move along screen horizontal
	var screen_right = Vector3.RIGHT
	var screen_up = Vector3.FORWARD

	if face_camera != null:
		screen_right = face_camera.face_screen_right
		screen_up = face_camera.face_screen_up
	else:
		# Fallback projection onto player_up
		var fwd = -global_transform.basis.z
		screen_up = (fwd - player_up * fwd.dot(player_up)).normalized()
		screen_right = screen_up.cross(player_up).normalized()

	# Read inputs as 4 cardinal directions:
	var input_x = 0.0 # Right (+1) / Left (-1)
	var input_y = 0.0 # Up (+1) / Down (-1)
	if Input.is_action_pressed("move_right"):
		input_x += 1.0
	if Input.is_action_pressed("move_left"):
		input_x -= 1.0
	if Input.is_action_pressed("move_forward"):
		input_y += 1.0
	if Input.is_action_pressed("move_back"):
		input_y -= 1.0

	var move_input = Vector2(input_x, input_y)
	var move_dir = Vector3.ZERO
	if move_input.length_squared() > 0.01:
		move_input = move_input.normalized()
		move_dir = (screen_right * move_input.x + screen_up * move_input.y).normalized()

		# Smoothly rotate character mesh to face movement direction on this face plane
		var cur_fwd = -global_transform.basis.z
		var cur_fwd_proj = (cur_fwd - player_up * cur_fwd.dot(player_up)).normalized()
		if cur_fwd_proj.length_squared() > 0.01:
			var target_basis = Basis()
			var desired_fwd = move_dir
			var desired_right = player_up.cross(desired_fwd).normalized()
			desired_fwd = desired_right.cross(player_up).normalized()
			var target_b = Basis(desired_right, player_up, -desired_fwd).orthonormalized()

			var cur_q = global_transform.basis.get_rotation_quaternion()
			var target_q = target_b.get_rotation_quaternion()
			global_transform.basis = Basis(cur_q.slerp(target_q, clamp(rotation_speed * delta, 0.0, 1.0)))

	# Split velocity into vertical and horizontal components relative to player's local up
	var vert_velocity = player_up * velocity.dot(player_up)
	var horiz_velocity = velocity - vert_velocity

	var target_horiz = move_dir * move_speed
	horiz_velocity = horiz_velocity.move_toward(target_horiz, acceleration * delta)

	velocity = horiz_velocity + vert_velocity

	# Update character's up_direction so is_on_floor() is calculated correctly
	up_direction = player_up
	floor_snap_length = 0.8
	floor_constant_speed = true
	floor_stop_on_slope = false

	move_and_slide()

func player_forward_dot(v: Vector3, up: Vector3) -> float:
	return v.dot(up)

func _align_to_normal(target_normal: Vector3, delta: float) -> void:
	current_up = current_up.slerp(target_normal, clamp(alignment_speed * delta, 0.0, 1.0)).normalized()

	# Current orientation
	var current_basis = global_transform.basis
	var forward = -current_basis.z

	# Project forward vector onto plane defined by new current_up
	forward = (forward - current_up * forward.dot(current_up)).normalized()
	if forward.length_squared() < 0.001:
		forward = current_basis.x.cross(current_up).normalized()
	var right = current_up.cross(forward).normalized()

	global_transform.basis = Basis(right, current_up, -forward).orthonormalized()
