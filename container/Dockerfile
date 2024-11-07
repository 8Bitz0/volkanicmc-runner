FROM docker.io/library/debian:bookworm-slim AS build

RUN apt-get update --quiet && \
    apt-get install --yes --quiet \
      bash \
      ca-certificates \
      curl \
      gcc \
      g++ && \
    # Delete package cache to avoid consuming space in layer
    apt-get clean && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /build

COPY ./src ./src
COPY ./Cargo.toml ./Cargo.toml
COPY ./Cargo.lock ./Cargo.lock

# Install Rust toolchain using rustup
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile=minimal --no-modify-path

# Set the default Rust environment
ENV PATH="${PATH}:/root/.cargo/bin"

RUN cargo build --profile release


FROM docker.io/library/debian:bookworm-slim

RUN apt-get update --quiet && \
    apt-get install --yes --quiet \
      bash \
      ca-certificates \
      curl \
      wget && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/*

COPY ./container/entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

# Add user `volkanic` ensuring the runner is not given root privileges
RUN useradd -s /bin/bash volkanic

# Create needed directories and set permissions
RUN mkdir /config && \
    mkdir /vk-store && \
    chown -R volkanic:volkanic /config && \
    chown -R volkanic:volkanic /vk-store

COPY --from=build /build/target/release/volkanicmc-runner /usr/bin/volkanicmc-runner

USER volkanic

ENTRYPOINT [ "/entrypoint.sh" ]