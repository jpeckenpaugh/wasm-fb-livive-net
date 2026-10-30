# Works with a native emsdk checkout or inside the container image.
# Container builds are canonical: `docker compose build serve`.
SHELL := /bin/bash

BUILD ?= build
GEN   := $(BUILD)/gen
OBJ   := $(BUILD)/obj

EMCC ?= emcc
CC   ?= cc

SRC         := src/main.c src/framebuffer.c src/media.c src/ui.c src/lcd.c \
               src/font5x7.c src/clock.c src/calc.c src/cal.c \
               src/fm.c src/preview.c src/markdown.c src/code.c

LDLIBS      ?= -lm
ASSET_UNITS := asset_0 asset_1 asset_2 asset_3 asset_4
GEN_UNITS   := assets_gen $(ASSET_UNITS)

WASM_OBJS := $(patsubst src/%.c,$(OBJ)/wasm/%.o,$(SRC)) \
             $(addprefix $(OBJ)/wasm/,$(addsuffix .o,$(GEN_UNITS))) \
             $(OBJ)/wasm/about_gen.o $(OBJ)/wasm/demo_gen.o

CFLAGS    ?= -std=c11 -Wall -Wextra -O2
INCLUDES  := -Isrc -I$(GEN)
WASM_LDFLAGS ?= \
  -O2 \
  -sMODULARIZE=1 -sEXPORT_ES6=1 -sEXPORT_NAME=createMediaBrowser \
  -sENVIRONMENT=web -sFILESYSTEM=0 \
  -sALLOW_MEMORY_GROWTH=0 -sINITIAL_MEMORY=64MB \
  -sEXPORTED_RUNTIME_METHODS=HEAPU8 \
  --no-entry

WASM_OUT := $(BUILD)/media_browser.mjs
V2_WASM_OUT := $(BUILD)/v2/appliance.wasm
ASSETS   := $(wildcard assets/*.png assets/*.jpg assets/*.jpeg)

NATIVE_COMMON := $(OBJ)/native/src/framebuffer.o
NATIVE_ASSETS := $(OBJ)/native/$(GEN)/assets_gen.o \
                 $(addprefix $(OBJ)/native/$(GEN)/,$(addsuffix .o,$(ASSET_UNITS))) \
                 $(OBJ)/native/$(GEN)/about_gen.o \
                 $(OBJ)/native/$(GEN)/demo_gen.o

.PHONY: all v2 test-v2 assets test clean debug release serve

all: $(WASM_OUT) $(V2_WASM_OUT)

v2: $(V2_WASM_OUT)

$(V2_WASM_OUT): $(wildcard v2/Cargo.toml v2/build.rs v2/src/*.rs)
	@mkdir -p $(dir $@)
	cargo build --manifest-path v2/Cargo.toml --target wasm32-unknown-unknown --release
	cp v2/target/wasm32-unknown-unknown/release/appliance.wasm $@
	@mkdir -p web/v2/build
	@cp $@ web/v2/build/appliance.wasm
	@ls -l $@ | awk '{printf "v2 wasm: %d bytes\n", $$5}'

test-v2:
	cargo test --manifest-path v2/Cargo.toml

# ABOUT.md is embedded as a C string so the Preview app renders the same file
# GitHub shows.
$(GEN)/about_gen.c: ABOUT.md tools/embed_about.py
	@mkdir -p $(GEN)
	python3 tools/embed_about.py --in ABOUT.md --out $(GEN)

$(GEN)/about_gen.h: $(GEN)/about_gen.c

# demo/* source samples, one per supported language.
$(GEN)/demo_gen.c: $(wildcard demo/*) tools/embed_demo.py
	@mkdir -p $(GEN)
	python3 tools/embed_demo.py --dir demo --out $(GEN)

$(GEN)/demo_gen.h: $(GEN)/demo_gen.c

# The generated header is the marker for the whole asset set.
$(GEN)/assets_gen.h: $(ASSETS) tools/prepare_assets.py
	@mkdir -p $(GEN)
	python3 tools/prepare_assets.py --assets assets --out $(GEN)

# Side-effect files: the script writes these after the header (see
# prepare_assets.py) so they are newer than the marker and this recipe does not
# run on a normal build. If one goes missing, drop the marker and regenerate.
GEN_CS := $(GEN)/assets_gen.c $(addprefix $(GEN)/,$(addsuffix .c,$(ASSET_UNITS)))

$(GEN_CS): $(GEN)/assets_gen.h
	@test -f $@ || { echo "regenerating missing $@"; rm -f $(GEN)/assets_gen.h; $(MAKE) --no-print-directory $(GEN)/assets_gen.h; }

$(OBJ)/wasm/%.o: src/%.c $(GEN)/assets_gen.h
	@mkdir -p $(dir $@)
	$(EMCC) $(CFLAGS) -MMD -MP $(INCLUDES) -c $< -o $@

$(OBJ)/wasm/%.o: $(GEN)/%.c $(GEN)/assets_gen.h
	@mkdir -p $(dir $@)
	$(EMCC) $(CFLAGS) -MMD -MP $(INCLUDES) -c $< -o $@

$(WASM_OUT): $(WASM_OBJS)
	$(EMCC) $^ -o $@ $(WASM_LDFLAGS)
	@ls -l $(BUILD)/media_browser.wasm | awk '{printf "wasm: %d bytes\n", $$5}'

$(OBJ)/native/%.o: %.c $(GEN)/assets_gen.h
	@mkdir -p $(dir $@)
	$(CC) $(CFLAGS) -MMD -MP $(INCLUDES) -Itests -c $< -o $@

# Object files used by the native test binaries, so depfiles cover every TU.
TEST_OBJS := $(OBJ)/native/tests/test_pixels.o $(OBJ)/native/tests/test_lcd.o \
             $(OBJ)/native/tests/test_clock.o $(OBJ)/native/tests/test_calc.o \
             $(OBJ)/native/tests/test_cal.o $(OBJ)/native/tests/test_fm.o \
             $(OBJ)/native/src/framebuffer.o $(OBJ)/native/src/media.o \
             $(OBJ)/native/src/ui.o $(OBJ)/native/src/clock.o \
             $(OBJ)/native/src/lcd.o $(OBJ)/native/src/font5x7.o \
             $(OBJ)/native/src/calc.o $(OBJ)/native/src/cal.o \
             $(OBJ)/native/src/fm.o $(OBJ)/native/src/preview.o \
             $(OBJ)/native/src/markdown.o $(OBJ)/native/src/code.o \
             $(NATIVE_ASSETS)

LCD_OBJS := $(OBJ)/native/src/lcd.o $(OBJ)/native/src/font5x7.o

# Header dependencies: without these, changing a struct (say clock_state_t)
# leaves stale objects compiled against the old layout.
-include $(patsubst %.o,%.d,$(WASM_OBJS) $(GEN_OBJS) $(TEST_OBJS))

$(BUILD)/test_pixels: $(OBJ)/native/tests/test_pixels.o \
                      $(OBJ)/native/src/media.o $(OBJ)/native/src/ui.o \
                      $(OBJ)/native/src/clock.o $(OBJ)/native/src/calc.o \
                      $(OBJ)/native/src/cal.o $(OBJ)/native/src/fm.o \
                      $(OBJ)/native/src/preview.o $(OBJ)/native/src/markdown.o \
                      $(OBJ)/native/src/code.o \
                      $(NATIVE_ASSETS) $(LCD_OBJS) \
                      $(NATIVE_ASSETS) $(NATIVE_COMMON)
	$(CC) $^ -o $@ $(LDLIBS)

$(BUILD)/test_lcd: $(OBJ)/native/tests/test_lcd.o $(LCD_OBJS) $(NATIVE_COMMON)
	$(CC) $^ -o $@ $(LDLIBS)

$(BUILD)/test_clock: $(OBJ)/native/tests/test_clock.o \
                     $(OBJ)/native/src/clock.o $(LCD_OBJS) $(NATIVE_COMMON)
	$(CC) $^ -o $@ $(LDLIBS)

$(BUILD)/test_calc: $(OBJ)/native/tests/test_calc.o $(OBJ)/native/src/calc.o \
                    $(LCD_OBJS) $(NATIVE_COMMON)
	$(CC) $^ -o $@ $(LDLIBS)

$(BUILD)/test_cal: $(OBJ)/native/tests/test_cal.o $(OBJ)/native/src/cal.o \
                   $(LCD_OBJS) $(NATIVE_COMMON)
	$(CC) $^ -o $@ $(LDLIBS)

$(BUILD)/test_fm: $(OBJ)/native/tests/test_fm.o $(OBJ)/native/src/fm.o \
                  $(OBJ)/native/src/code.o $(LCD_OBJS) $(NATIVE_ASSETS) $(NATIVE_COMMON)
	$(CC) $^ -o $@ $(LDLIBS)

$(BUILD)/test_code: $(OBJ)/native/tests/test_code.o $(OBJ)/native/src/code.o \
                    $(LCD_OBJS) $(NATIVE_COMMON)
	$(CC) $^ -o $@ $(LDLIBS)

test: $(BUILD)/test_pixels $(BUILD)/test_lcd $(BUILD)/test_clock $(BUILD)/test_calc $(BUILD)/test_cal $(BUILD)/test_fm $(BUILD)/test_code
	@for t in $^; do ./$$t; done
	@$(MAKE) --no-print-directory test-v2

# Force regeneration after editing assets/.
assets:
	rm -rf $(GEN)
	$(MAKE) $(GEN)/assets_gen.h

debug:
	$(MAKE) BUILD=build-debug \
	  CFLAGS="-std=c11 -Wall -Wextra -O0 -g" \
	  WASM_LDFLAGS="$(WASM_LDFLAGS) -O0 -g -sASSERTIONS=1" all

release: all

serve:
	docker compose up --build serve

clean:
	rm -rf $(BUILD) build-debug web/v2/build v2/target
