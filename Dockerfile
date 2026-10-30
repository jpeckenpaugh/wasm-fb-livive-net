# Reproducible build: pinned Emscripten + pinned Pillow, all artifacts produced
# inside the image. `serve` is the canonical demo.
ARG EMSDK_VERSION=6.0.13
FROM emscripten/emsdk:${EMSDK_VERSION} AS toolchain

# Asset preprocessing is the only place a Python dependency is needed. Pinning
# it keeps the generated C byte-identical between machines.
RUN python3 -m pip install --no-cache-dir --break-system-packages Pillow==11.3.0

# Install a pinned minimal Rust + wasm32-unknown-unknown toolchain for `/v2/`
ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH="/usr/local/cargo/bin:${PATH}"
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | \
    sh -s -- -y --default-toolchain 1.85.0 --profile minimal --target wasm32-unknown-unknown && \
    chmod -R a+w /usr/local/rustup /usr/local/cargo

WORKDIR /src

# Warm the sysroot into an image layer so incremental builds are seconds, not
# minutes. Failure here is not fatal: it just means a cold cache on first build.
RUN embuilder build sysroot --quiet || true

FROM toolchain AS build
COPY Makefile ./
COPY ABOUT.md ./
COPY demo ./demo
COPY tools ./tools
COPY assets ./assets
COPY src ./src
COPY tests ./tests
COPY v2 ./v2
RUN make -j"$(nproc)" all test

# Portable artifact extraction:
#   docker build --target artifacts --output out .
FROM scratch AS artifacts
COPY --from=build /src/build/ /

FROM nginx:1.29-alpine AS serve
COPY --from=build /src/build/media_browser.mjs /usr/share/nginx/html/build/
COPY --from=build /src/build/media_browser.wasm /usr/share/nginx/html/build/
COPY --from=build /src/build/v2/appliance.wasm /usr/share/nginx/html/v2/build/
COPY assets /usr/share/nginx/html/assets/
COPY demo /usr/share/nginx/html/demo/
COPY web /usr/share/nginx/html/
COPY docker/nginx.conf /etc/nginx/conf.d/default.conf
