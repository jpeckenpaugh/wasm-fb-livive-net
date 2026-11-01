extends SceneTree

# Headless rule/topology test entry point.
#
# Run with:
#   godot --headless --path . --fixed-fps 60 --script res://tests/run_tests.gd
# or:
#   make test
#
# Exits 0 only when every suite passes. `make test` additionally requires the
# "TEST RESULT: PASS" marker and rejects any SCRIPT ERROR in the output, so an
# aborted suite or a script error can never masquerade as success.
#
# Suites run in declaration order and are awaited, so a suite may yield with
# `await tree.process_frame` (the finale lifecycle suite does). `--fixed-fps 60`
# makes those awaits deterministic and fast.

const Check = preload("res://tests/check.gd")

const SUITES: Array = [
	preload("res://tests/test_grid_topology.gd"),
	preload("res://tests/test_gem_placement.gd"),
	preload("res://tests/test_face_numbering.gd"),
	preload("res://tests/test_resolution_accounting.gd"),
	preload("res://tests/test_controller_flow.gd"),
	preload("res://tests/test_prepopulation.gd"),
	preload("res://tests/test_finale_lifecycle.gd"),
]

const WATCHDOG_MS := 60000

var _started_ms: int = 0
var _finished: bool = false

func _initialize() -> void:
	_started_ms = Time.get_ticks_msec()
	# Deferred: nodes added to the tree from _initialize() do not receive
	# _ready()/@onready initialization in Godot 4.7.
	_run.call_deferred()

func _process(_delta: float) -> bool:
	if not _finished and Time.get_ticks_msec() - _started_ms > WATCHDOG_MS:
		_finished = true
		print("TEST RESULT: FAIL (watchdog timeout after %d ms)" % WATCHDOG_MS)
		quit(1)
	return false

func _run() -> void:
	print("Godot %s" % Engine.get_version_info()["string"])
	var t = Check.new()
	t.tree = self

	for suite_script in SUITES:
		var suite = suite_script.new()
		var before: int = t.checks
		var suite_started: int = Time.get_ticks_msec()
		print("\n== %s ==" % suite.suite_name())
		# Suites may be synchronous or await process_frame (finale lifecycle).
		# Awaiting a non-coroutine returns immediately.
		await suite.run(t)
		print("-- %s: %d checks (%d ms)" % [
			suite.suite_name(), t.checks - before, Time.get_ticks_msec() - suite_started
		])

	_finished = true
	print("\nchecks=%d failures=%d" % [t.checks, t.failures])
	if t.failures == 0:
		print("TEST RESULT: PASS (%d checks)" % t.checks)
		quit(0)
	else:
		print("TEST RESULT: FAIL (%d of %d checks failed)" % [t.failures, t.checks])
		quit(1)
