# Builder and runtime share Debian bookworm so the glibc the binary links against exists at runtime.
FROM rust:1.99-slim-bookworm AS build
WORKDIR /src
# Deps first with stub sources: this layer is reused until Cargo.toml/Cargo.lock change.
# src-tauri (the Mac app) is a workspace member: cargo needs its manifest, never builds it here.
COPY Cargo.toml Cargo.lock ./
COPY src-tauri/Cargo.toml src-tauri/
RUN mkdir -p src src-tauri/src && echo 'fn main() {}' > src/main.rs && touch src/lib.rs src-tauri/src/lib.rs \
 && cargo build --release && rm -rf src
COPY migrations migrations
COPY templates templates
COPY static static
COPY src src
# touch: the stub build left newer mtimes than the copied sources, cargo would skip the rebuild.
RUN touch src/main.rs src/lib.rs && cargo build --release

# No shell or package manager at runtime: smaller image, less to patch. The empty data dir is
# made in the build stage because distroless can't mkdir; nonroot = uid 65532.
RUN mkdir /data-empty
FROM gcr.io/distroless/cc-debian12:nonroot
WORKDIR /app
COPY --from=build /src/target/release/atharu /usr/local/bin/atharu
COPY --from=build --chown=65532:65532 /data-empty /app/data
EXPOSE 7070
CMD ["/usr/local/bin/atharu"]
