FROM lukemathwalker/cargo-chef:latest AS chef
WORKDIR /usr/src/nowplaying-ttv

ARG PORT
ENV PORT=$PORT
ARG INTERNAL_PORT
ENV INTERNAL_PORT=$INTERNAL_PORT

FROM chef AS prepare
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS build
COPY --from=prepare /usr/src/nowplaying-ttv/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release

EXPOSE $PORT
EXPOSE $INTERNAL_PORT

FROM rust AS runtime
COPY --from=build /usr/src/nowplaying-ttv/target/release/nowplaying-ttv .
EXPOSE 3000
CMD ["./nowplaying-ttv"]