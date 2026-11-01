extends Control

## Unfolded cube-net progress panel for Gem Miner.
## Read-only view over VolumetricGrid state. Stakes are deliberately neutral:
## every marked cell renders the same amber X, whether a gem or rock is under it.
## Pops a one-shot gold pulse when a face is first surveyed; never mutates
## gameplay state or tile nodes.

const FACE_COUNT := 6
const FACE_DIM := 10
const FACE_CELLS := FACE_DIM * FACE_DIM

# 9 px mini-cells keep the 4x3 cross clear of the projected 3D cube.
const CELL := 9.0
const CELL_GAP := 1.0
const FACE_PX := 99.0
const H_FACE_GAP := 10.0
const V_ROW_GAP := 8.0
const CAPTION_H := 15.0
const NET_W := 426.0
const NET_H := 358.0
const LEGEND_H := 34.0
const SLOT_STEP_X := FACE_PX + H_FACE_GAP
const SLOT_STEP_Y := FACE_PX + CAPTION_H + V_ROW_GAP
const LEGEND_COL_W := NET_W / 3.0

const POLL_SEC := 0.1
const PULSE_SEC := 0.8

const S_ROCK := 0
const S_OPEN := 1
const S_STAKED := 2
const S_CRACKED := 3

const C_GOLD := Color(0.98, 0.82, 0.38, 1.0)
const C_MUTED := Color(0.58, 0.64, 0.72, 1.0)
const C_CURRENT := Color(0.32, 0.84, 1.0, 1.0)
const C_LOST := Color(0.90, 0.25, 0.22, 1.0)
const C_ROCK := Color(0.145, 0.155, 0.185, 1.0)
const C_OPEN := Color(0.47, 0.43, 0.37, 1.0)
const C_STAKED_FILL := Color(0.20, 0.16, 0.10, 1.0)
const C_CRACKED_FILL := Color(0.24, 0.10, 0.10, 1.0)
const C_STAKED := Color(1.0, 0.72, 0.24, 1.0)
const C_CRACKED := Color(0.93, 0.27, 0.20, 1.0)
const C_FACE_BG := Color(0.028, 0.033, 0.043, 1.0)

@export var controller_path: NodePath = NodePath("../../../..")

var _controller: VolumetricCubeController
var _snap: PackedByteArray = PackedByteArray()
var _face_rock_open: PackedInt32Array = PackedInt32Array()
var _face_stakes: PackedInt32Array = PackedInt32Array()
var _complete_bits: int = 0
var _pulses: Dictionary = {}
var _signature: int = -1
var _poll_accum: float = 0.0
var _was_populated: bool = false
var _was_won: bool = false
var _was_pulsing: bool = false
var _net_disabled: bool = false
var _font: Font

@onready var _title_label: Label = get_node_or_null("../TitleRow/NetTitleLabel") as Label
@onready var _footer_label: Label = get_node_or_null("../NetFooterLabel") as Label

func _ready() -> void:
	var win := get_window()
	if win != null:
		win.title = "Gem Miner — Geode Claim"

	_font = ThemeDB.fallback_font
	_snap.resize(FACE_COUNT * FACE_CELLS)
	_face_rock_open.resize(FACE_COUNT)
	_face_stakes.resize(FACE_COUNT)

	_controller = get_node_or_null(controller_path) as VolumetricCubeController
	if _controller == null:
		_controller = get_tree().current_scene as VolumetricCubeController
	if _controller == null:
		push_warning("ClaimNet: controller not found")
		set_process(false)
		return

	# Signals exist on the controller instance immediately; its @onready nodes do not.
	_controller.face_surveyed.connect(_on_face_reported)
	_controller.claim_complete.connect(_on_claim_complete)
	_wire_reset_hooks.call_deferred()

func _wire_reset_hooks() -> void:
	if not is_instance_valid(_controller):
		return
	var restart := _controller.restart_button
	if is_instance_valid(restart) and not restart.pressed.is_connected(_on_new_game):
		restart.pressed.connect(_on_new_game)
	var difficulty := _controller.difficulty_option
	if is_instance_valid(difficulty) and not difficulty.item_selected.is_connected(_on_difficulty_selected):
		difficulty.item_selected.connect(_on_difficulty_selected)

func _on_new_game() -> void:
	_clear_transient()
	_poll(true)

func _on_difficulty_selected(_index: int) -> void:
	_clear_transient()
	_poll(true)

func _on_face_reported(face: int) -> void:
	if _net_disabled or not is_instance_valid(_controller):
		return
	if face < 0 or face >= FACE_COUNT:
		return
	_pulses[face] = Time.get_ticks_msec()
	_poll(true)

func _on_claim_complete() -> void:
	_poll(true)

func _clear_transient() -> void:
	_pulses.clear()
	_complete_bits = 0
	_was_pulsing = false
	_signature = -1
	queue_redraw()

func _disable_net(grid_size: int) -> void:
	if _net_disabled:
		return
	_net_disabled = true
	push_warning("ClaimNet: expected a %dx%d grid, got %dx%d; cube net disabled" % [
		FACE_DIM, FACE_DIM, grid_size, grid_size
	])
	_set_label(_title_label, "CLAIM SURVEY · UNAVAILABLE", C_MUTED)
	_set_label(_footer_label, "10×10 grid required (found %d×%d)" % [grid_size, grid_size], C_MUTED)
	_pulses.clear()
	_complete_bits = 0
	_signature = -1
	set_process(false)
	queue_redraw()

func _process(delta: float) -> void:
	_poll_accum += delta
	if _poll_accum >= POLL_SEC:
		_poll_accum = 0.0
		_poll(false)

	var pulsing := _update_pulses()
	if pulsing:
		_was_pulsing = true
		queue_redraw()
	elif _was_pulsing:
		_was_pulsing = false
		queue_redraw()

func _update_pulses() -> bool:
	if _pulses.is_empty():
		return false
	var now := Time.get_ticks_msec()
	var active := false
	for face in _pulses.keys():
		if now - int(_pulses[face]) >= int(PULSE_SEC * 1000.0):
			_pulses.erase(face)
		else:
			active = true
	return active

# Polls a 600-cell snapshot, hashes it, and only redraws when something changed.
func _poll(force: bool) -> void:
	if _net_disabled or not is_instance_valid(_controller):
		return
	var grid := _controller.grid
	if grid == null or grid.all_cells.is_empty():
		return
	if grid.size != FACE_DIM:
		_disable_net(grid.size)
		return

	var populated: bool = grid.gems_populated
	if _was_populated and not populated:
		_clear_transient()
		force = true
	_was_populated = populated

	var won: bool = _controller.game_won
	if _was_won and not won:
		_clear_transient()
		force = true
	_was_won = won
	var lost: bool = _controller.game_over
	var view: int = _controller.current_viewed_face

	var board := grid.get_board_stats()
	var complete_bits := 0
	var faces_complete := 0
	for f in range(FACE_COUNT):
		var fs := grid.get_face_stats(f)
		_face_rock_open[f] = int(fs["face_rock_excavated"])
		_face_stakes[f] = int(fs["face_gems_staked"]) + int(fs["face_gems_cracked"]) + int(fs["face_misplaced_marks"])
		if fs["is_face_complete"]:
			complete_bits |= 1 << f
			faces_complete += 1

	var sig: int = 2166136261
	var can_excavate := false
	for f in range(FACE_COUNT):
		var face_base: int = f * FACE_CELLS
		for v in range(FACE_DIM):
			var row_base: int = face_base + v * FACE_DIM
			for u in range(FACE_DIM):
				var cell = grid.get_cell_uv(f, u, v)
				var st := _classify(cell)
				_snap[row_base + u] = st
				sig = ((sig ^ st) * 16777619) & 0xFFFFFFFF
				if cell != null and not cell.is_gem and not cell.is_revealed and not cell.is_marked:
					can_excavate = true

	var stakes_placed: int = int(board["gems_staked"]) + int(board["gems_cracked"]) + int(board["misplaced_marks"])
	var secured_gems: int = int(board["gems_staked"]) + int(board["gems_cracked"])
	var expected_gems: int = _controller.total_gems
	var expected_rock: int = max(grid.all_cells.size() - expected_gems, 0)
	var stuck: bool = populated and not won and not lost and not board["is_complete"] and not can_excavate

	if lost and not won:
		_pulses.clear()
	else:
		for face in _pulses.keys():
			if not (complete_bits & (1 << int(face))):
				_pulses.erase(face)

	var flags: int = view
	if won:
		flags |= 1 << 3
	if lost:
		flags |= 1 << 4
	if populated:
		flags |= 1 << 5
	if stuck:
		flags |= 1 << 6
	flags |= complete_bits << 7
	sig = ((sig ^ flags) * 16777619) & 0xFFFFFFFF

	if not force and sig == _signature:
		return
	_signature = sig
	_complete_bits = complete_bits

	_refresh_chrome({
		"populated": populated,
		"won": won,
		"lost": lost,
		"stuck": stuck,
		"rock_excavated": int(board["rock_excavated"]),
		"rock_total": int(board["rock_total"]),
		"stakes_placed": stakes_placed,
		"faces_complete": faces_complete,
		"secured_gems": secured_gems,
		"expected_rock": expected_rock,
		"expected_gems": expected_gems,
	})
	queue_redraw()

func _classify(cell: VolumetricGrid.CellData) -> int:
	if cell == null:
		return S_ROCK
	if cell.is_detonated:
		return S_CRACKED
	if cell.is_marked:
		# Deliberately ignore is_gem: every stake looks identical while live.
		return S_STAKED
	if cell.is_revealed:
		return S_OPEN
	# Unrevealed gems deliberately look like unrevealed rock.
	return S_ROCK

func _refresh_chrome(state: Dictionary) -> void:
	var title := "CLAIM SURVEY · UNSURVEYED"
	var title_color := C_MUTED
	if state["won"]:
		title = "CLAIM COMPLETE"
		title_color = C_GOLD
	elif state["lost"]:
		title = "CLAIM LOST"
		title_color = C_LOST
	elif state["populated"]:
		title = "CLAIM SURVEY · %d/%d" % [state["faces_complete"], FACE_COUNT]
		title_color = C_GOLD
	_set_label(_title_label, title, title_color)

	var footer := ""
	var footer_color := C_MUTED
	if state["won"]:
		footer = "All six faces surveyed · %d gems secured" % state["secured_gems"]
		footer_color = C_GOLD
	elif state["lost"]:
		footer = "Pick integrity depleted · New Claim to try again"
		footer_color = C_LOST
	elif not state["populated"]:
		footer = "First pick is safe · %d rock · %d gems expected" % [
			state["expected_rock"], state["expected_gems"]
		]
	elif state["stuck"]:
		footer = "No further rock can be excavated — adjust stakes to finish."
	else:
		footer = "Rock excavated %d/%d · Stakes placed %d · Faces %d/%d" % [
			state["rock_excavated"], state["rock_total"], state["stakes_placed"],
			state["faces_complete"], FACE_COUNT
		]
	_set_label(_footer_label, footer, footer_color)

func _set_label(label: Label, text: String, color: Color) -> void:
	if label == null:
		return
	if label.text != text:
		label.text = text
	label.add_theme_color_override("font_color", color)

func _draw() -> void:
	if _net_disabled or not is_instance_valid(_controller):
		return
	if _snap.size() != FACE_COUNT * FACE_CELLS:
		return
	if _font == null:
		_font = ThemeDB.fallback_font
		if _font == null:
			return

	var origin := Vector2(
		max(floor((size.x - NET_W) * 0.5), 0.0),
		max(floor((size.y - (NET_H + LEGEND_H)) * 0.5), 0.0)
	)
	var populated: bool = _controller.grid.gems_populated
	var lost: bool = _controller.game_over
	var won: bool = _controller.game_won
	var suppress: bool = lost and not won

	for f in range(FACE_COUNT):
		_draw_face(f, origin, populated, lost, won, suppress)
	_draw_legend(origin + Vector2(0.0, NET_H + 8.0))
	_draw_state_frame(origin, lost, won)

func _draw_face(face: int, origin: Vector2, populated: bool, lost: bool, won: bool, suppress: bool) -> void:
	var slot := origin + _face_slot(face)
	var grid_pos := slot + Vector2(0.0, CAPTION_H)
	var grid_rect := Rect2(grid_pos, Vector2(FACE_PX, FACE_PX))
	draw_rect(grid_rect.grow(2.0), C_FACE_BG)

	var face_base: int = face * FACE_CELLS
	for v in range(FACE_DIM):
		var row_base: int = face_base + v * FACE_DIM
		for u in range(FACE_DIM):
			var st: int = _snap[row_base + u]
			var r := Rect2(
				grid_pos + Vector2(u * (CELL + CELL_GAP), v * (CELL + CELL_GAP)),
				Vector2(CELL, CELL)
			)
			draw_rect(r, _state_fill(st))
			if st == S_STAKED:
				_draw_cross(r, C_STAKED)
			elif st == S_CRACKED:
				_draw_cross(r, C_CRACKED)

	var view: int = _controller.current_viewed_face
	var is_current: bool = face == view and not lost and not won
	if is_current:
		draw_rect(grid_rect.grow(1.0), C_CURRENT, false, 2.0)

	var complete: bool = bool(_complete_bits & (1 << face))
	var caption_color := C_MUTED
	if complete and not suppress:
		caption_color = C_GOLD
	if is_current:
		caption_color = C_CURRENT

	var name: String = VolumetricGrid.FACE_NAMES[face].to_upper()
	var text_x := slot.x + 1.0
	draw_string(_font, Vector2(text_x, slot.y + 11.0), name, HORIZONTAL_ALIGNMENT_LEFT, -1, 9, caption_color)
	text_x += _font.get_string_size(name, HORIZONTAL_ALIGNMENT_LEFT, -1, 9).x + 3.0

	if not populated:
		draw_string(_font, Vector2(text_x, slot.y + 11.0), "—", HORIZONTAL_ALIGNMENT_LEFT, -1, 9, caption_color)
	else:
		var rock_text := "%d" % _face_rock_open[face]
		draw_string(_font, Vector2(text_x, slot.y + 11.0), rock_text, HORIZONTAL_ALIGNMENT_LEFT, -1, 9, caption_color)
		text_x += _font.get_string_size(rock_text, HORIZONTAL_ALIGNMENT_LEFT, -1, 9).x + 3.0
		var stake_text := "%d×" % _face_stakes[face]
		draw_string(_font, Vector2(text_x, slot.y + 11.0), stake_text, HORIZONTAL_ALIGNMENT_LEFT, -1, 9, C_STAKED)
		text_x += _font.get_string_size(stake_text, HORIZONTAL_ALIGNMENT_LEFT, -1, 9).x + 2.0

	if complete and not suppress:
		var check_x: float = min(text_x, slot.x + FACE_PX - 8.0)
		_draw_check(Vector2(check_x, slot.y + 2.0), C_GOLD)

	if not suppress and complete and _pulses.has(face):
		var elapsed := (Time.get_ticks_msec() - int(_pulses[face])) / 1000.0
		var t: float = clamp(elapsed / PULSE_SEC, 0.0, 1.0)
		if t < 1.0:
			var grow := 2.0 + 8.0 * t
			var col := Color(C_GOLD, 1.0 - t)
			draw_rect(
				Rect2(grid_rect.position - Vector2(grow, grow),
				grid_rect.size + Vector2(grow * 2.0, grow * 2.0)),
				col, false, 2.0
			)

func _draw_legend(pos: Vector2) -> void:
	var states := [S_ROCK, S_OPEN, S_STAKED, S_CRACKED, -1, -2]
	var labels := ["Rock", "Excavated", "Stake", "Cracked gem", "Viewed face", "Face surveyed"]
	for i in range(states.size()):
		var row := 0 if i < 3 else 1
		var col := i % 3
		var p := pos + Vector2(col * LEGEND_COL_W, row * 14.0)
		var st: int = states[i]
		var swatch := Rect2(p, Vector2(9.0, 9.0))
		if st == -1:
			draw_rect(swatch, C_FACE_BG)
			draw_rect(swatch, C_CURRENT, false, 1.0)
		elif st == -2:
			draw_rect(swatch, C_FACE_BG)
			_draw_check(Vector2(p.x + 1.0, p.y + 1.0), C_GOLD)
		else:
			draw_rect(swatch, _state_fill(st))
			if st == S_STAKED:
				_draw_cross(swatch, C_STAKED)
			elif st == S_CRACKED:
				_draw_cross(swatch, C_CRACKED)
		var text_color := C_CURRENT if st == -1 else C_MUTED
		draw_string(_font, Vector2(p.x + 13.0, p.y + 9.0), labels[i],
			HORIZONTAL_ALIGNMENT_LEFT, -1, 10, text_color)

func _draw_state_frame(origin: Vector2, lost: bool, won: bool) -> void:
	if not lost and not won:
		return
	var col := C_LOST if lost else C_GOLD
	draw_rect(
		Rect2(origin - Vector2(2.0, 2.0), Vector2(NET_W + 4.0, NET_H + LEGEND_H + 4.0)),
		col, false, 2.0
	)

func _face_slot(face: int) -> Vector2:
	match face:
		0: return Vector2(SLOT_STEP_X, SLOT_STEP_Y)       # FRONT
		1: return Vector2(SLOT_STEP_X * 2.0, SLOT_STEP_Y) # RIGHT
		2: return Vector2(SLOT_STEP_X * 3.0, SLOT_STEP_Y) # BACK
		3: return Vector2(0.0, SLOT_STEP_Y)               # LEFT
		4: return Vector2(SLOT_STEP_X, 0.0)               # TOP
		5: return Vector2(SLOT_STEP_X, SLOT_STEP_Y * 2.0) # BOTTOM
	return Vector2.ZERO

func _state_fill(st: int) -> Color:
	match st:
		S_OPEN: return C_OPEN
		S_STAKED: return C_STAKED_FILL
		S_CRACKED: return C_CRACKED_FILL
		_: return C_ROCK

func _draw_cross(rect: Rect2, color: Color) -> void:
	var pad := 2.0
	var p := rect.position
	var s := rect.size
	draw_line(p + Vector2(pad, pad), p + s - Vector2(pad, pad), color, 1.0)
	draw_line(p + Vector2(s.x - pad, pad), p + Vector2(pad, s.y - pad), color, 1.0)

func _draw_check(pos: Vector2, color: Color) -> void:
	draw_line(pos + Vector2(0.0, 4.0), pos + Vector2(3.0, 7.0), color, 1.5)
	draw_line(pos + Vector2(3.0, 7.0), pos + Vector2(8.0, 1.0), color, 1.5)
