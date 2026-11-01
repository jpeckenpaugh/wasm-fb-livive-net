@tool
class_name DodecahedronPlanet
extends StaticBody3D

@export var radius: float = 20.0:
	set(val):
		radius = max(val, 1.0)
		if is_node_ready():
			generate_planet()

@export var face_color: Color = Color(0.12, 0.16, 0.22, 0.95):
	set(val):
		face_color = val
		if is_node_ready():
			generate_planet()

@export var wireframe_color: Color = Color(0.3, 0.8, 1.0, 1.0):
	set(val):
		wireframe_color = val
		if is_node_ready():
			generate_planet()

var mesh_instance: MeshInstance3D
var collision_shape: CollisionShape3D
var wireframe_instance: ImmediateMesh
var wireframe_mesh_node: MeshInstance3D

var face_normals: Array[Vector3] = []
var face_centers: Array[Vector3] = []
var face_ups: Array[Vector3] = []
var face_adjacency: Dictionary = {} # Dictionary of int -> Dictionary of int -> Vector3 (edge midpoint in local)

func _ready() -> void:
	generate_planet()

func get_closest_face_index(world_pos: Vector3) -> int:
	var local_pos = to_local(world_pos)
	var best_idx: int = 0
	var best_dot: float = -999999.0
	for i in range(face_normals.size()):
		var d = face_normals[i].dot(local_pos.normalized())
		if d > best_dot:
			best_dot = d
			best_idx = i
	return best_idx

# Returns camera transform oriented so the crossed edge (between from_face and to_face) is at bottom of viewport
func get_face_camera_transform_from_transition(from_face: int, to_face: int, cam_distance: float, tilt_angle_deg: float = 32.0) -> Transform3D:
	if to_face < 0 or to_face >= face_normals.size():
		return Transform3D.IDENTITY

	var n = (global_transform.basis * face_normals[to_face]).normalized()
	var c = global_transform * face_centers[to_face]

	# Determine canonical up vector for this face view
	var u: Vector3
	if from_face >= 0 and face_adjacency.has(to_face) and face_adjacency[to_face].has(from_face):
		# Shared edge midpoint
		var edge_mid_local: Vector3 = face_adjacency[to_face][from_face]
		var edge_mid_world: Vector3 = global_transform * edge_mid_local
		# 'u' points from the bottom crossed edge toward face center (upwards on screen)
		u = (c - edge_mid_world).normalized()
	else:
		u = (global_transform.basis * face_ups[to_face]).normalized()

	var r = n.cross(u).normalized()
	u = r.cross(n).normalized() # Ensure strict orthogonality with face normal

	# Position camera pulled along face normal, tilted backwards toward the crossed edge (-u)
	var tilt_rad = deg_to_rad(tilt_angle_deg)
	var cam_dir = (n * cos(tilt_rad) - u * sin(tilt_rad)).normalized()
	var cam_pos = c + cam_dir * cam_distance

	# Orient camera looking at face center with 'u' as screen up (so crossed edge is bottom of screen)
	var forward = (c - cam_pos).normalized() # Camera -Z
	var cam_right = forward.cross(u).normalized()
	var cam_up = cam_right.cross(forward).normalized()

	return Transform3D(Basis(cam_right, cam_up, -forward), cam_pos)

func get_face_camera_transform(face_idx: int, cam_distance: float, tilt_angle_deg: float = 32.0) -> Transform3D:
	return get_face_camera_transform_from_transition(-1, face_idx, cam_distance, tilt_angle_deg)

func generate_planet() -> void:
	face_normals.clear()
	face_centers.clear()
	face_ups.clear()

	# Clean up previous generated nodes if any
	for child in get_children():
		child.queue_free()

	var phi = (1.0 + sqrt(5.0)) / 2.0 # Golden ratio ~1.618

	# Raw 20 vertices of regular dodecahedron
	var base_verts: Array[Vector3] = [
		# (+-1, +-1, +-1) -> 8 vertices (0..7)
		Vector3(-1, -1, -1), # 0
		Vector3(-1, -1,  1), # 1
		Vector3(-1,  1, -1), # 2
		Vector3(-1,  1,  1), # 3
		Vector3( 1, -1, -1), # 4
		Vector3( 1, -1,  1), # 5
		Vector3( 1,  1, -1), # 6
		Vector3( 1,  1,  1), # 7
		# (0, +-1/phi, +-phi) -> 4 vertices (8..11)
		Vector3(0, -1.0/phi, -phi), # 8
		Vector3(0, -1.0/phi,  phi), # 9
		Vector3(0,  1.0/phi, -phi), # 10
		Vector3(0,  1.0/phi,  phi), # 11
		# (+-1/phi, +-phi, 0) -> 4 vertices (12..15)
		Vector3(-1.0/phi, -phi, 0), # 12
		Vector3(-1.0/phi,  phi, 0), # 13
		Vector3( 1.0/phi, -phi, 0), # 14
		Vector3( 1.0/phi,  phi, 0), # 15
		# (+-phi, 0, +-1/phi) -> 4 vertices (16..19)
		Vector3(-phi, 0, -1.0/phi), # 16
		Vector3(-phi, 0,  1.0/phi), # 17
		Vector3( phi, 0, -1.0/phi), # 18
		Vector3( phi, 0,  1.0/phi), # 19
	]

	# Normalize so vertices lie on sphere of given radius
	var vertices: Array[Vector3] = []
	for v in base_verts:
		vertices.append(v.normalized() * radius)

	# 12 pentagonal faces (vertex indices ordered CCW viewed from exterior)
	var face_indices: Array[Array] = [
		[3, 11, 7, 15, 13],    # Top face
		[0, 12, 4, 8, 16],     # Bottom face
		[11, 9, 5, 7, 19],     # Face 3
		[3, 17, 1, 9, 11],     # Face 4
		[2, 10, 8, 16, 13],    # Face 5
		[2, 13, 15, 6, 10],    # Face 6
		[6, 15, 7, 19, 18],    # Face 7
		[6, 18, 4, 8, 10],     # Face 8
		[1, 12, 0, 16, 17],    # Face 9
		[1, 9, 5, 14, 12],     # Face 10
		[5, 19, 18, 4, 14],    # Face 11
		[0, 8, 10, 2, 16]      # (Ensure standard orientation below)
	]

	face_adjacency.clear()
	for i in range(12):
		face_adjacency[i] = {}

	# Precompute shared edges between adjacent faces
	for i in range(12):
		for j in range(i + 1, 12):
			var shared_indices = []
			for vi in face_indices[i]:
				if vi in face_indices[j]:
					shared_indices.append(vi)
			if shared_indices.size() == 2:
				var mid = (vertices[shared_indices[0]] + vertices[shared_indices[1]]) * 0.5
				face_adjacency[i][j] = mid
				face_adjacency[j][i] = mid

	# Construct faces cleanly by computing planar normal and sorting vertices CCW
	var face_planes: Array[Array] = _build_correct_faces(vertices)

	# 1. Build Solid ArrayMesh for rendering faces
	var st = SurfaceTool.new()
	st.begin(Mesh.PRIMITIVE_TRIANGLES)

	# Build distinct edges set for wireframe
	var edge_set = {}

	for f in face_planes:
		var v0: Vector3 = f[0]
		var v1: Vector3 = f[1]
		var v2: Vector3 = f[2]
		var normal: Vector3 = ((v1 - v0).cross(v2 - v0)).normalized()

		# Compute center of this pentagon
		var center = Vector3.ZERO
		for pt in f:
			center += pt
		center /= f.size()

		# Determine canonical 'up' tangent on this face (project world UP, or forward if parallel)
		var proj_up = Vector3.UP - normal * Vector3.UP.dot(normal)
		if proj_up.length_squared() < 0.01:
			proj_up = Vector3.FORWARD - normal * Vector3.FORWARD.dot(normal)
		var face_up = proj_up.normalized()

		face_normals.append(normal)
		face_centers.append(center)
		face_ups.append(face_up)

		# Fan triangulation for pentagon: (0,1,2), (0,2,3), (0,3,4)
		for i in range(1, 4):
			st.set_normal(normal)
			st.add_vertex(f[0])
			st.set_normal(normal)
			st.add_vertex(f[i])
			st.set_normal(normal)
			st.add_vertex(f[i + 1])

		for i in range(f.size()):
			var va: Vector3 = f[i]
			var vb: Vector3 = f[(i + 1) % f.size()]
			var key = _edge_key(va, vb)
			edge_set[key] = [va, vb]

	var solid_mesh = st.commit()

	var mat = StandardMaterial3D.new()
	mat.albedo_color = face_color
	mat.roughness = 0.7
	mat.cull_mode = BaseMaterial3D.CULL_DISABLED
	mat.specular_mode = BaseMaterial3D.SPECULAR_SCHLICK_GGX

	mesh_instance = MeshInstance3D.new()
	mesh_instance.name = "DodecaMesh"
	mesh_instance.mesh = solid_mesh
	mesh_instance.material_override = mat
	add_child(mesh_instance)

	# 2. Build Wireframe Mesh
	var wire_st = SurfaceTool.new()
	wire_st.begin(Mesh.PRIMITIVE_LINES)
	for key in edge_set:
		var edge = edge_set[key]
		# Slightly lift wireframe outward so it doesn't z-fight with faces
		var normal_a = edge[0].normalized() * 0.05
		var normal_b = edge[1].normalized() * 0.05
		wire_st.set_color(wireframe_color)
		wire_st.add_vertex(edge[0] + normal_a)
		wire_st.set_color(wireframe_color)
		wire_st.add_vertex(edge[1] + normal_b)

	var wire_mesh = wire_st.commit()
	var wire_mat = StandardMaterial3D.new()
	wire_mat.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	wire_mat.albedo_color = wireframe_color

	wireframe_mesh_node = MeshInstance3D.new()
	wireframe_mesh_node.name = "WireframeMesh"
	wireframe_mesh_node.mesh = wire_mesh
	wireframe_mesh_node.material_override = wire_mat
	add_child(wireframe_mesh_node)

	# 3. Collision Shape
	collision_shape = CollisionShape3D.new()
	collision_shape.name = "DodecaCollision"
	var convex_shape = ConvexPolygonShape3D.new()
	var point_cloud: PackedVector3Array = PackedVector3Array()
	for v in vertices:
		point_cloud.append(v)
	convex_shape.points = point_cloud
	collision_shape.shape = convex_shape
	add_child(collision_shape)

func _edge_key(a: Vector3, b: Vector3) -> String:
	var pa = "%0.2f,%0.2f,%0.2f" % [a.x, a.y, a.z]
	var pb = "%0.2f,%0.2f,%0.2f" % [b.x, b.y, b.z]
	if pa < pb:
		return pa + ":" + pb
	return pb + ":" + pa

func _build_correct_faces(vertices: Array[Vector3]) -> Array[Array]:
	# Canonical faces for regular dodecahedron with 20 vertices:
	# Vertices indices:
	# 0:(-1,-1,-1), 1:(-1,-1,1), 2:(-1,1,-1), 3:(-1,1,1),
	# 4:(1,-1,-1), 5:(1,-1,1), 6:(1,1,-1), 7:(1,1,1),
	# 8:(0,-1/p,-p), 9:(0,-1/p,p), 10:(0,1/p,-p), 11:(0,1/p,p),
	# 12:(-1/p,-p,0), 13:(-1/p,p,0), 14:(1/p,-p,0), 15:(1/p,p,0),
	# 16:(-p,0,-1/p), 17:(-p,0,1/p), 18:(p,0,-1/p), 19:(p,0,1/p)
	var face_indices: Array[Array] = [
		[3, 11, 7, 15, 13],
		[7, 19, 5, 9, 11],
		[3, 13, 2, 16, 17],
		[1, 17, 16, 0, 12],
		[1, 9, 5, 14, 12],
		[5, 19, 18, 4, 14],
		[7, 15, 6, 18, 19],
		[2, 10, 6, 15, 13],
		[6, 10, 8, 4, 18],
		[4, 8, 0, 12, 14],
		[0, 8, 10, 2, 16],
		[3, 17, 1, 9, 11]
	]

	var result: Array[Array] = []
	for indices in face_indices:
		var face_pts: Array[Vector3] = []
		for idx in indices:
			face_pts.append(vertices[idx])

		# Ensure outward winding order:
		var center = Vector3.ZERO
		for p in face_pts:
			center += p
		center /= face_pts.size()

		var normal = (face_pts[1] - face_pts[0]).cross(face_pts[2] - face_pts[0]).normalized()
		if normal.dot(center) < 0:
			face_pts.reverse()

		result.append(face_pts)
	return result
