class_name MinesweeperTile3D
extends Area3D

signal tile_clicked(x: int, y: int, button_index: int)

var grid_x: int = 0
var grid_y: int = 0
var is_mine: bool = false
var neighbor_mines: int = 0
var is_revealed: bool = false
var is_flagged: bool = false

var mesh_instance: MeshInstance3D
var label_3d: Label3D
var flag_marker: Node3D
var mine_marker: Node3D
var base_material: StandardMaterial3D
var original_y: float = 0.0

const NUMBER_COLORS = [
	Color(0.2, 0.4, 1.0),   # 1: Blue
	Color(0.2, 0.8, 0.2),   # 2: Green
	Color(1.0, 0.2, 0.2),   # 3: Red
	Color(0.3, 0.1, 0.7),   # 4: Dark Blue / Purple
	Color(0.6, 0.1, 0.1),   # 5: Maroon
	Color(0.1, 0.7, 0.7),   # 6: Cyan
	Color(0.1, 0.1, 0.1),   # 7: Black
	Color(0.5, 0.5, 0.5)    # 8: Gray
]

const COLOR_DEFAULT = Color(0.20, 0.22, 0.28)
const COLOR_HOVER = Color(0.32, 0.36, 0.46)
const COLOR_FLAGGED = Color(0.18, 0.72, 0.28)

func setup(gx: int, gy: int) -> void:
	grid_x = gx
	grid_y = gy
	name = "Tile_%d_%d" % [gx, gy]

	# 1. Collision Shape (Real 3D Cube: 0.94x0.94x0.94)
	var col = CollisionShape3D.new()
	var box_shape = BoxShape3D.new()
	box_shape.size = Vector3(0.94, 0.94, 0.94)
	col.shape = box_shape
	add_child(col)

	# 2. Main Visual Cube Mesh (1x1x1 discrete block)
	mesh_instance = MeshInstance3D.new()
	var box_mesh = BoxMesh.new()
	box_mesh.size = Vector3(0.94, 0.94, 0.94)
	mesh_instance.mesh = box_mesh

	base_material = StandardMaterial3D.new()
	base_material.albedo_color = COLOR_DEFAULT
	base_material.roughness = 0.55
	base_material.metallic = 0.15
	mesh_instance.material_override = base_material
	add_child(mesh_instance)

	# 3. Number Label3D (on top face of mini-cube)
	label_3d = Label3D.new()
	label_3d.transform.origin = Vector3(0, 0.50, 0)
	label_3d.rotation_degrees = Vector3(-90, 0, 0) # Lay flat facing up along local Y
	label_3d.font_size = 54
	label_3d.pixel_size = 0.009
	label_3d.double_sided = true
	label_3d.no_depth_test = true
	label_3d.render_priority = 10
	label_3d.outline_size = 8
	label_3d.outline_modulate = Color(0, 0, 0, 0.9)
	label_3d.visible = false
	add_child(label_3d)

	# 4. Mine Visual (Black Spiky Sphere)
	mine_marker = Node3D.new()
	mine_marker.name = "MineMarker"
	mine_marker.visible = false

	var mine_sphere = MeshInstance3D.new()
	var sph = SphereMesh.new()
	sph.radius = 0.22
	sph.height = 0.44
	mine_sphere.mesh = sph
	mine_sphere.transform.origin = Vector3(0, 0.52, 0)
	var mine_mat = StandardMaterial3D.new()
	mine_mat.albedo_color = Color(0.1, 0.1, 0.12)
	mine_mat.metallic = 0.8
	mine_mat.roughness = 0.3
	mine_sphere.material_override = mine_mat
	mine_marker.add_child(mine_sphere)

	add_child(mine_marker)

	# Mouse signal connections
	input_event.connect(_on_input_event)
	mouse_entered.connect(_on_mouse_entered)
	mouse_exited.connect(_on_mouse_exited)
	original_y = position.y

var press_timer: float = 0.0
var is_pressing_left: bool = false
var long_press_triggered: bool = false
const LONG_PRESS_THRESHOLD: float = 0.35 # seconds

signal tile_pressed(x: int, y: int)

func _process(delta: float) -> void:
	if is_pressing_left and not long_press_triggered:
		press_timer += delta
		if press_timer >= LONG_PRESS_THRESHOLD:
			long_press_triggered = true
			is_pressing_left = false
			# Trigger secondary click action (Right Click)
			tile_clicked.emit(grid_x, grid_y, MOUSE_BUTTON_RIGHT)

func _on_input_event(_camera: Node, event: InputEvent, _pos: Vector3, _normal: Vector3, _shape_idx: int) -> void:
	if event is InputEventMouseButton:
		if event.button_index == MOUSE_BUTTON_LEFT:
			if event.pressed:
				# Immediate tactile impulse upon pressing down
				tile_pressed.emit(grid_x, grid_y)
				# Check if Ctrl or Cmd is held (Mac convention for right click)
				if event.ctrl_pressed or event.meta_pressed:
					tile_clicked.emit(grid_x, grid_y, MOUSE_BUTTON_RIGHT)
				else:
					is_pressing_left = true
					long_press_triggered = false
					press_timer = 0.0
			else:
				# Left mouse button released
				if is_pressing_left and not long_press_triggered:
					tile_clicked.emit(grid_x, grid_y, MOUSE_BUTTON_LEFT)
				is_pressing_left = false
		elif event.button_index == MOUSE_BUTTON_RIGHT and event.pressed:
			tile_pressed.emit(grid_x, grid_y)
			tile_clicked.emit(grid_x, grid_y, MOUSE_BUTTON_RIGHT)

func _on_mouse_entered() -> void:
	if not is_revealed:
		if is_flagged:
			base_material.albedo_color = Color(0.24, 0.85, 0.35)
		else:
			base_material.albedo_color = COLOR_HOVER
		translate_object_local(Vector3(0, 0.08, 0))

func _on_mouse_exited() -> void:
	is_pressing_left = false
	if not is_revealed:
		if is_flagged:
			base_material.albedo_color = COLOR_FLAGGED
		else:
			base_material.albedo_color = COLOR_DEFAULT
		translate_object_local(Vector3(0, -0.08, 0))

func toggle_flag() -> void:
	if is_revealed:
		return
	is_flagged = not is_flagged
	if is_flagged:
		base_material.albedo_color = COLOR_FLAGGED
		base_material.emission_enabled = true
		base_material.emission = Color(0.12, 0.55, 0.20)
		base_material.emission_energy_multiplier = 0.6
	else:
		base_material.albedo_color = COLOR_DEFAULT
		base_material.emission_enabled = false

func reveal(exploded: bool = false) -> void:
	if is_revealed:
		return
	is_revealed = true
	is_flagged = false
	base_material.emission_enabled = false
	translate_object_local(Vector3(0, -0.12, 0)) # Lower tile slightly into the cube face

	if is_mine:
		mine_marker.visible = true
		if exploded:
			base_material.albedo_color = Color(0.85, 0.2, 0.15)
			base_material.emission_enabled = true
			base_material.emission = Color(1.0, 0.2, 0.1)
			base_material.emission_energy_multiplier = 2.0
		else:
			base_material.albedo_color = Color(0.3, 0.15, 0.15)
	else:
		# Empty or numbered
		base_material.albedo_color = Color(0.15, 0.17, 0.2)
		if neighbor_mines > 0:
			label_3d.text = str(neighbor_mines)
			var color_idx = clamp(neighbor_mines - 1, 0, NUMBER_COLORS.size() - 1)
			label_3d.modulate = NUMBER_COLORS[color_idx]
			label_3d.visible = true
