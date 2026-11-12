# Builder and runtime share Debian bookworm so the glibc the binary links against exists at runtime.
FROM rust:1.99-slim-bookworm AS build
WORKDIR /src
# Deps first with stub sources: this layer is reused until Cargo.toml/Cargo.lock change.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs && touch src/lib.rs \
 && cargo build --release && rm -rf src
COPY migrations migrations
COPY templates templates
COPY static static
COPY src src
# touch: the stub build left newer mtimes than the copied sources, cargo would skip the rebuild.
RUN touch src/main.rs src/lib.rs && cargo build --release

FROM debian:bookworm-slim
RUN useradd --uid 1000 --create-home worklog
WORKDIR /app
COPY --from=build /src/target/release/worklog /usr/local/bin/worklog
RUN mkdir data && chown worklog data
USER worklog
EXPOSE 7070
CMD ["worklog"]
