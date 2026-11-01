class_name Minesweeper3D
extends Node3D

@export var initial_size: int = 10
@export var mine_count_per_face: int = 10
@export var tile_spacing: float = 1.0

# 6-step peel sequence on a 10x10x10 cube:
# Step 0: Front face (10x10). Cube volume is 10x10x10 (w=10, h=10, d=10).
#         Peeling Front removes Z-layer: remaining volume is 10x10x9 (X=10, Y=10, Z=9).
#         Rotating Right (around Y) brings the Right face (+X) to Front (+Z):
#         New axes facing camera: X_new = Z_old (9), Y_new = Y_old (10), Z_new = X_old (10).
# Step 1: Right face (9x10). Cube volume is 9x10x10 (w=9, h=10, d=10).
#         Peeling removes front Z-layer: remaining volume is 9x10x9 (X=9, Y=10, Z=9).
#         Rotating Up (around X) brings the Top face (+Y) to Front (+Z):
#         New axes facing camera: X_new = X_old (9), Y_new = Z_old (9), Z_new = Y_old (10).
# Step 2: Top face (9x9). Cube volume is 9x9x10 (w=9, h=9, d=10).
#         Peeling removes front Z-layer: remaining volume is 9x9x9 (X=9, Y=9, Z=9).
#         Rotating Up (around X) brings Back face to Front:
#         New axes facing camera: X_new = 9, Y_new = 9, Z_new = 9.
# Step 3: Back face (9x9). Cube volume is 9x9x9 (w=9, h=9, d=9).
#         Peeling removes front Z-layer: remaining volume is 9x9x8 (X=9, Y=9, Z=8).
#         Rotating Up (around X) brings Bottom face to Front:
#         New axes facing camera: X_new = 9, Y_new = 8, Z_new = 9.
# Step 4: Bottom face (9x8). Cube volume is 9x8x9 (w=9, h=8, d=9).
#         Peeling removes front Z-layer: remaining volume is 9x8x8 (X=9, Y=8, Z=8).
#         Rotating Right (around Y) brings Left face to Front:
#         New axes facing camera: X_new = 8, Y_new = 8, Z_new = 9.
# Step 5: Left face (8x8). Cube volume is 8x8x9 (w=8, h=8, d=9).
#         Peeling leaves the core 8x8x8 cube!
const STEP_CONFIG = [
	{"name": "Face 1 / 6: Front",  "rot_axis": "right", "desc": "Rotating Right..."},
	{"name": "Face 2 / 6: Right",  "rot_axis": "up",    "desc": "Rotating Up..."},
	{"name": "Face 3 / 6: Top",    "rot_axis": "up",    "desc": "Rotating Up..."},
	{"name": "Face 4 / 6: Back",   "rot_axis": "up",    "desc": "Rotating Up..."},
	{"name": "Face 5 / 6: Bottom", "rot_axis": "right", "desc": "Rotating Right..."},
	{"name": "Face 6 / 6: Left",   "rot_axis": "none",  "desc": "Completed!"}
]

# Exact dimensions of the active face (w x h) and depth (d) of the cube volume at each step
const STEP_DIMENSIONS = [
	{"w": 10, "h": 10, "d": 10},
	{"w": 9,  "h": 10, "d": 10},
	{"w": 9,  "h": 9,  "d": 10},
	{"w": 9,  "h": 9,  "d": 9},
	{"w": 9,  "h": 8,  "d": 9},
	{"w": 8,  "h": 8,  "d": 9}
]

var current_step: int = 0
var is_transitioning: bool = false
var game_over: bool = false
var game_won: bool = false

const MAX_STRIKES: int = 3
var strikes: int = 0

var current_face_node: Node3D
var active_tiles: Array = [] # 2D array [col][row]
var mines_placed: bool = false
var tiles_revealed_count: int = 0
var flags_placed_count: int = 0

var active_w: int = 10
var active_h: int = 10
var active_d: int = 10

@onready var cube_root: Node3D = $CubeRoot
@onready var status_label: Label = $UI/Panel/VBoxContainer/StatusLabel
@onready var mines_label: Label = $UI/Panel/VBoxContainer/MinesLabel
@onready var restart_button: Button = $UI/Panel/VBoxContainer/RestartButton
@onready var camera: Camera3D = $Camera3D

var core_multimesh_instance: MultiMeshInstance3D

func _ready() -> void:
	Input.set_mouse_mode(Input.MOUSE_MODE_VISIBLE)
	DisplayServer.window_set_mode(DisplayServer.WINDOW_MODE_MAXIMIZED)
	restart_button.pressed.connect(reset_game)
	reset_game()

func _unhandled_input(event: InputEvent) -> void:
	if event is InputEventKey and event.pressed:
		if event.keycode == KEY_F11 or event.keycode == KEY_F:
			var current_mode = DisplayServer.window_get_mode()
			if current_mode == DisplayServer.WINDOW_MODE_FULLSCREEN or current_mode == DisplayServer.WINDOW_MODE_EXCLUSIVE_FULLSCREEN:
				DisplayServer.window_set_mode(DisplayServer.WINDOW_MODE_MAXIMIZED)
			else:
				DisplayServer.window_set_mode(DisplayServer.WINDOW_MODE_FULLSCREEN)

func reset_game() -> void:
	current_step = 0
	is_transitioning = false
	game_over = false
	game_won = false
	strikes = 0

	cube_root.transform = Transform3D.IDENTITY

	for child in cube_root.get_children():
		child.queue_free()

	# Create MultiMeshInstance3D for authentic 1x1x1 cubes in the remaining cube core
	core_multimesh_instance = MultiMeshInstance3D.new()
	core_multimesh_instance.name = "CoreMultiMesh"
	cube_root.add_child(core_multimesh_instance)

	# Frame camera straight-on at the active front-facing face
	camera.keep_aspect = Camera3D.KEEP_HEIGHT
	camera.fov = 48.0
	camera.position = Vector3(0, 0, 18.0)
	camera.look_at(Vector3(0, 0, 0), Vector3.UP)

	_build_active_face()

func _build_active_face() -> void:
	mines_placed = false
	tiles_revealed_count = 0
	flags_placed_count = 0
	active_tiles.clear()

	# Ensure cube_root transform is strictly identity for the active face
	cube_root.transform = Transform3D.IDENTITY

	var dims = STEP_DIMENSIONS[current_step]
	active_w = dims["w"]
	active_h = dims["h"]
	active_d = dims["d"]

	# Rebuild the core cubes behind the active slice:
	# Dimensions: active_w x active_h x (active_d - 1)
	var remaining_depth = active_d - 1
	var total_core_cubes = active_w * active_h * remaining_depth

	var mm = MultiMesh.new()
	mm.transform_format = MultiMesh.TRANSFORM_3D
	mm.instance_count = total_core_cubes

	var box = BoxMesh.new()
	box.size = Vector3(0.94, 0.94, 0.94) # Exact same size as active 1x1x1 cubes
	var mat = StandardMaterial3D.new()
	mat.albedo_color = Color(0.20, 0.22, 0.28)
	mat.roughness = 0.55
	mat.metallic = 0.15
	box.material = mat
	mm.mesh = box

	# Center the entire remaining cube volume (w x h x d) around (0, 0, 0)
	# Center of cube in Z is 0.
	# Z goes from -active_d * 0.5 to +active_d * 0.5
	# The active front slice is at z = (active_d - 1) * 0.5 * tile_spacing
	# Core slices are at z = ((active_d - 1) * 0.5 - 1 - z_layer) * tile_spacing
	var offset_x = (active_w - 1) * tile_spacing * 0.5
	var offset_y = (active_h - 1) * tile_spacing * 0.5
	var front_slice_z = (active_d - 1) * tile_spacing * 0.5

	var idx = 0
	for z_layer in range(remaining_depth):
		var cz = front_slice_z - (z_layer + 1) * tile_spacing
		for col in range(active_w):
			var cx = col * tile_spacing - offset_x
			for row in range(active_h):
				var cy = (active_h - 1 - row) * tile_spacing - offset_y
				var t = Transform3D(Basis.IDENTITY, Vector3(cx, cy, cz))
				mm.set_instance_transform(idx, t)
				idx += 1

	core_multimesh_instance.multimesh = mm

	# Create active face container
	current_face_node = Node3D.new()
	current_face_node.name = "ActiveFace_%d" % current_step
	cube_root.add_child(current_face_node)

	# Lay mini-cubes on the front slice (+Z) centered at (0, 0, front_slice_z)
	for col in range(active_w):
		var col_tiles = []
		var tile_x = col * tile_spacing - offset_x
		for row in range(active_h):
			var tile_y = (active_h - 1 - row) * tile_spacing - offset_y
			var tile = MinesweeperTile3D.new()
			tile.position = Vector3(tile_x, tile_y, front_slice_z)
			# Rotation: local +Y points +Z (toward camera), local -Z points +Y (up)
			tile.rotation_degrees = Vector3(90, 0, 0)
			tile.setup(col, row)
			tile.tile_clicked.connect(_on_tile_clicked)
			tile.tile_pressed.connect(_on_tile_pressed)
			current_face_node.add_child(tile)
			col_tiles.append(tile)
		active_tiles.append(col_tiles)

	# Position camera centered on the active face at front_slice_z
	var target_cam_z = front_slice_z + 13.0
	camera.position = Vector3(0, 0, target_cam_z)
	camera.look_at(Vector3(0, 0, front_slice_z), Vector3.UP)

	_update_ui("Click any cube to start " + STEP_CONFIG[current_step]["name"])

var elastic_tween: Tween

func _on_tile_pressed(x: int, y: int) -> void:
	if game_over or game_won or is_transitioning:
		return
	_apply_click_elasticity(x, y)

func _on_tile_clicked(x: int, y: int, button_index: int) -> void:
	if game_over or game_won or is_transitioning:
		return

	var tile: MinesweeperTile3D = active_tiles[x][y]

	if button_index == MOUSE_BUTTON_RIGHT:
		if not tile.is_revealed:
			tile.toggle_flag()
			flags_placed_count += 1 if tile.is_flagged else -1
			_update_ui("Marked tile 🟢" if tile.is_flagged else "Unmarked tile")
			_check_face_win_condition()
		elif tile.is_revealed and tile.neighbor_mines > 0:
			_chord_reveal_neighbors(x, y)
	elif button_index == MOUSE_BUTTON_LEFT:
		if tile.is_flagged or tile.is_revealed:
			return

		if not mines_placed:
			_place_mines_for_active_face(x, y)
			mines_placed = true

		if tile.is_mine:
			_on_mine_detonated(tile)
		else:
			_reveal_tile(x, y)
			_check_face_win_condition()

func _apply_click_elasticity(col: int, row: int) -> void:
	if is_transitioning:
		return

	# Normalized click coordinate from face center in range [-1.0, 1.0]
	var half_w = max((active_w - 1) * 0.5, 0.5)
	var half_h = max((active_h - 1) * 0.5, 0.5)
	var norm_x = (col - half_w) / half_w
	var norm_y = (half_h - row) / half_h # +1.0 is top, -1.0 is bottom

	# Clicking top (norm_y > 0) causes top to recede (-X pitch rotation)
	# Clicking right (norm_x > 0) causes right to recede (+Y yaw rotation)
	var max_tilt_deg = 3.0 # Tasteful, subtle tactile flex
	var pitch = -norm_y * max_tilt_deg
	var yaw = norm_x * max_tilt_deg

	var flex_rot = Vector3(pitch, yaw, 0.0)
	var flex_pos_z = -0.15 # Subtle cushion recoil into screen

	if elastic_tween and elastic_tween.is_valid():
		elastic_tween.kill()

	elastic_tween = create_tween()
	# Phase 1: Snappy tactile impulse into pressed quadrant (80ms)
	elastic_tween.parallel().tween_property(cube_root, "rotation_degrees", flex_rot, 0.08).set_trans(Tween.TRANS_QUAD).set_ease(Tween.EASE_OUT)
	elastic_tween.parallel().tween_property(cube_root, "position:z", flex_pos_z, 0.08).set_trans(Tween.TRANS_QUAD).set_ease(Tween.EASE_OUT)

	# Phase 2: Clean spring-back settling smoothly without wild wobbling (260ms)
	elastic_tween.chain().tween_property(cube_root, "rotation_degrees", Vector3.ZERO, 0.26).set_trans(Tween.TRANS_BACK).set_ease(Tween.EASE_OUT)
	elastic_tween.parallel().tween_property(cube_root, "position:z", 0.0, 0.26).set_trans(Tween.TRANS_BACK).set_ease(Tween.EASE_OUT)

func _place_mines_for_active_face(safe_x: int, safe_y: int) -> void:
	var potential_positions: Array[Vector2i] = []
	for x in range(active_w):
		for y in range(active_h):
			if abs(x - safe_x) <= 1 and abs(y - safe_y) <= 1:
				continue
			potential_positions.append(Vector2i(x, y))

	potential_positions.shuffle()
	var to_place = min(mine_count_per_face, potential_positions.size())
	for i in range(to_place):
		var pos = potential_positions[i]
		active_tiles[pos.x][pos.y].is_mine = true

	# Calculate neighbor numbers for active face
	for x in range(active_w):
		for y in range(active_h):
			var tile: MinesweeperTile3D = active_tiles[x][y]
			if not tile.is_mine:
				tile.neighbor_mines = _count_neighbor_mines(x, y)

func _count_neighbor_mines(cx: int, cy: int) -> int:
	var count = 0
	for dx in [-1, 0, 1]:
		for dy in [-1, 0, 1]:
			if dx == 0 and dy == 0:
				continue
			var nx = cx + dx
			var ny = cy + dy
			if nx >= 0 and nx < active_w and ny >= 0 and ny < active_h:
				if active_tiles[nx][ny].is_mine:
					count += 1
	return count

func _reveal_tile(x: int, y: int) -> void:
	var queue: Array[Vector2i] = [Vector2i(x, y)]

	while queue.size() > 0:
		var current = queue.pop_front()
		var t: MinesweeperTile3D = active_tiles[current.x][current.y]

		if t.is_revealed or t.is_flagged:
			continue

		t.reveal()
		tiles_revealed_count += 1

		if t.neighbor_mines == 0 and not t.is_mine:
			for dx in [-1, 0, 1]:
				for dy in [-1, 0, 1]:
					if dx == 0 and dy == 0:
						continue
					var nx = current.x + dx
					var ny = current.y + dy
					if nx >= 0 and nx < active_w and ny >= 0 and ny < active_h:
						var neighbor: MinesweeperTile3D = active_tiles[nx][ny]
						if not neighbor.is_revealed and not neighbor.is_flagged:
							queue.append(Vector2i(nx, ny))

func _chord_reveal_neighbors(cx: int, cy: int) -> void:
	var center_tile: MinesweeperTile3D = active_tiles[cx][cy]
	var accounted_mines_count = 0
	var unrevealed_unflagged: Array[Vector2i] = []

	for dx in [-1, 0, 1]:
		for dy in [-1, 0, 1]:
			if dx == 0 and dy == 0:
				continue
			var nx = cx + dx
			var ny = cy + dy
			if nx >= 0 and nx < active_w and ny >= 0 and ny < active_h:
				var neighbor: MinesweeperTile3D = active_tiles[nx][ny]
				# Account for marked (flagged) mines as well as already detonated/revealed mines
				if neighbor.is_flagged or (neighbor.is_revealed and neighbor.is_mine):
					accounted_mines_count += 1
				elif not neighbor.is_revealed:
					unrevealed_unflagged.append(Vector2i(nx, ny))

	if accounted_mines_count == center_tile.neighbor_mines and unrevealed_unflagged.size() > 0:
		for pos in unrevealed_unflagged:
			var n_tile: MinesweeperTile3D = active_tiles[pos.x][pos.y]
			if n_tile.is_mine:
				_on_mine_detonated(n_tile)
				if game_over:
					break
			else:
				_reveal_tile(pos.x, pos.y)

		if not game_over:
			_check_face_win_condition()

func _on_mine_detonated(hit_tile: MinesweeperTile3D) -> void:
	if hit_tile.is_revealed:
		return
	strikes += 1
	hit_tile.reveal(true)

	# Quick camera shake
	var shake_tween = create_tween().set_trans(Tween.TRANS_SINE).set_ease(Tween.EASE_IN_OUT)
	var original_cam_pos = camera.position
	shake_tween.tween_property(camera, "position", original_cam_pos + Vector3(0.25, -0.2, 0), 0.05)
	shake_tween.tween_property(camera, "position", original_cam_pos + Vector3(-0.25, 0.2, 0), 0.05)
	shake_tween.tween_property(camera, "position", original_cam_pos + Vector3(0.15, 0.1, 0), 0.05)
	shake_tween.tween_property(camera, "position", original_cam_pos, 0.05)

	if strikes >= MAX_STRIKES:
		game_over = true
		# Reveal all other mines on this face
		for col in active_tiles:
			for tile in col:
				if tile.is_mine and tile != hit_tile:
					tile.reveal(false)
		_update_ui("💥 STRIKE 3! Game Over on " + STEP_CONFIG[current_step]["name"] + " 💥")
	else:
		_update_ui("⚠️ STRIKE %d / %d! Be careful!" % [strikes, MAX_STRIKES])

func _check_face_win_condition() -> void:
	var total_safe_tiles = (active_w * active_h) - mine_count_per_face

	if tiles_revealed_count >= total_safe_tiles:
		# Auto-flag remaining mines
		for col in active_tiles:
			for tile in col:
				if tile.is_mine and not tile.is_flagged:
					tile.toggle_flag()

		if current_step < 5:
			_execute_slice_dissolve_and_rotation()
		else:
			game_won = true
			_update_ui("🏆 VICTORY! All 6 Faces Cleared! Full 10x10x10 Cube Solved! 🎉")

func _execute_slice_dissolve_and_rotation() -> void:
	is_transitioning = true
	if elastic_tween and elastic_tween.is_valid():
		elastic_tween.kill()
	cube_root.position.z = 0.0

	var step_cfg = STEP_CONFIG[current_step]
	_update_ui("Face Cleared! Dissolving slice & " + step_cfg["desc"])

	var old_face_node = current_face_node

	# 1. Slice Dissolve Animation: cubes pop forward slightly and shrink to zero
	var dissolve_tween = create_tween().set_parallel(true).set_trans(Tween.TRANS_QUAD).set_ease(Tween.EASE_OUT)
	for col in active_tiles:
		for tile in col:
			dissolve_tween.tween_property(tile, "position:z", tile.position.z + 0.4, 0.4)
			dissolve_tween.tween_property(tile, "scale", Vector3.ZERO, 0.9)

	# 2. Re-center the remaining block volume during transition:
	# Prior to peeling, the volume was active_w x active_h x active_d with front at front_slice_z.
	# The core multimesh is currently positioned at (0, 0, 0) representing the core slices.
	# As the front slice dissolves, the remaining block's depth is (active_d - 1).
	# Its new center along Z shifts backwards by half a tile (0.5 * tile_spacing).
	# We smoothly tween core_multimesh_instance.position.z so there is no visual snap!
	var rot_axis_str = step_cfg["rot_axis"]
	var target_basis = cube_root.basis
	if rot_axis_str == "right":
		target_basis = Basis(Vector3.UP, -PI * 0.5) * cube_root.basis
	elif rot_axis_str == "up":
		target_basis = Basis(Vector3.RIGHT, PI * 0.5) * cube_root.basis

	# Camera perspective target for the incoming face
	var next_dims = STEP_DIMENSIONS[current_step + 1]
	var next_front_slice_z = (next_dims["d"] - 1) * tile_spacing * 0.5
	var target_cam_pos = Vector3(0, 0, next_front_slice_z + 13.0)

	var rot_tween = create_tween().set_parallel(true).set_trans(Tween.TRANS_CUBIC).set_ease(Tween.EASE_IN_OUT)
	var start_q = cube_root.basis.get_rotation_quaternion()
	var end_q = target_basis.get_rotation_quaternion()

	rot_tween.tween_method(func(t: float):
		cube_root.basis = Basis(start_q.slerp(end_q, t)), 0.0, 1.0, 1.2
	)
	rot_tween.tween_property(camera, "position", target_cam_pos, 1.2)

	# Smoothly shift the core so its new center becomes the rotation pivot
	rot_tween.tween_property(core_multimesh_instance, "position:z", 0.5 * tile_spacing, 1.2)

	rot_tween.chain().tween_callback(func():
		old_face_node.queue_free()
		core_multimesh_instance.position = Vector3.ZERO
		current_step += 1
		_build_active_face()
		is_transitioning = false
	)

func _update_ui(status: String) -> void:
	status_label.text = status
	var strikes_str = ""
	for i in range(MAX_STRIKES):
		if i < strikes:
			strikes_str += "❌ "
		else:
			strikes_str += "💚 "

	mines_label.text = "%s (%dx%d) | Mines: %d | Flags: %d | Strikes: %s" % [
		STEP_CONFIG[current_step]["name"], active_w, active_h, mine_count_per_face, flags_placed_count, strikes_str.strip_edges()
	]
