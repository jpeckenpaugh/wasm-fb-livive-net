# Reproducible build: pinned Emscripten + pinned Pillow, all artifacts produced
# inside the image. `serve` is the canonical demo.
ARG EMSDK_VERSION=6.0.13
FROM emscripten/emsdk:${EMSDK_VERSION} AS toolchain

# Asset preprocessing is the only place a Python dependency is needed. Pinning
# it keeps the generated C byte-identical between machines.
RUN python3 -m pip install --no-cache-dir --break-system-packages Pillow==11.3.0

WORKDIR /src

# Warm the sysroot into an image layer so incremental builds are seconds, not
# minutes. Failure here is not fatal: it just means a cold cache on first build.
RUN embuilder build sysroot --quiet || true

FROM toolchain AS build
COPY Makefile ./
COPY tools ./tools
COPY assets ./assets
COPY src ./src
COPY tests ./tests
RUN make -j"$(nproc)" all test

# Portable artifact extraction:
#   docker build --target artifacts --output out .
FROM scratch AS artifacts
COPY --from=build /src/build/ /

FROM nginx:1.29-alpine AS serve
COPY --from=build /src/build/media_browser.mjs /usr/share/nginx/html/build/
COPY --from=build /src/build/media_browser.wasm /usr/share/nginx/html/build/
COPY web /usr/share/nginx/html/
COPY docker/nginx.conf /etc/nginx/conf.d/default.conf
