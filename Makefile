# Works with a native emsdk checkout or inside the container image.
# Container builds are canonical: `docker compose build serve`.
SHELL := /bin/bash

BUILD ?= build
GEN   := $(BUILD)/gen
OBJ   := $(BUILD)/obj

EMCC ?= emcc
CC   ?= cc

SRC         := src/main.c src/framebuffer.c src/media.c src/ui.c src/lcd.c \
               src/clock.c src/calc.c

LDLIBS      ?= -lm
ASSET_UNITS := asset_0 asset_1 asset_2 asset_3 asset_4
GEN_UNITS   := assets_gen $(ASSET_UNITS)

WASM_OBJS := $(patsubst src/%.c,$(OBJ)/wasm/%.o,$(SRC)) \
             $(addprefix $(OBJ)/wasm/,$(addsuffix .o,$(GEN_UNITS)))

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
ASSETS   := $(wildcard assets/*.png assets/*.jpg assets/*.jpeg)

NATIVE_COMMON := $(OBJ)/native/src/framebuffer.o
NATIVE_ASSETS := $(OBJ)/native/$(GEN)/assets_gen.o \
                 $(addprefix $(OBJ)/native/$(GEN)/,$(addsuffix .o,$(ASSET_UNITS)))

.PHONY: all assets test clean debug release serve

all: $(WASM_OUT)

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
	$(EMCC) $(CFLAGS) $(INCLUDES) -c $< -o $@

$(OBJ)/wasm/%.o: $(GEN)/%.c $(GEN)/assets_gen.h
	@mkdir -p $(dir $@)
	$(EMCC) $(CFLAGS) $(INCLUDES) -c $< -o $@

$(WASM_OUT): $(WASM_OBJS)
	$(EMCC) $^ -o $@ $(WASM_LDFLAGS)
	@ls -l $(BUILD)/media_browser.wasm | awk '{printf "wasm: %d bytes\n", $$5}'

$(OBJ)/native/%.o: %.c $(GEN)/assets_gen.h
	@mkdir -p $(dir $@)
	$(CC) $(CFLAGS) $(INCLUDES) -Itests -c $< -o $@

$(BUILD)/test_pixels: $(OBJ)/native/tests/test_pixels.o \
                      $(OBJ)/native/src/media.o $(OBJ)/native/src/ui.o \
                      $(OBJ)/native/src/clock.o $(OBJ)/native/src/lcd.o \
                      $(OBJ)/native/src/calc.o \
                      $(NATIVE_ASSETS) $(NATIVE_COMMON)
	$(CC) $^ -o $@ $(LDLIBS)

$(BUILD)/test_lcd: $(OBJ)/native/tests/test_lcd.o \
                   $(OBJ)/native/src/lcd.o $(NATIVE_COMMON)
	$(CC) $^ -o $@

$(BUILD)/test_clock: $(OBJ)/native/tests/test_clock.o \
                     $(OBJ)/native/src/clock.o $(OBJ)/native/src/lcd.o \
                     $(NATIVE_COMMON)
	$(CC) $^ -o $@ $(LDLIBS)

$(BUILD)/test_calc: $(OBJ)/native/tests/test_calc.o $(OBJ)/native/src/calc.o \
                    $(OBJ)/native/src/lcd.o $(NATIVE_COMMON)
	$(CC) $^ -o $@ $(LDLIBS)

test: $(BUILD)/test_pixels $(BUILD)/test_lcd $(BUILD)/test_clock $(BUILD)/test_calc
	@for t in $^; do ./$$t; done

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
	rm -rf $(BUILD) build-debug
