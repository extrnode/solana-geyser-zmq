from ubuntu:20.04

# Set TZ
ENV TZ=Europe/Kyiv
RUN ln -snf /usr/share/zoneinfo/$TZ /etc/localtime && echo $TZ > /etc/timezone

RUN apt-get update && apt install -y curl gcc g++ cmake gcc-multilib libpq-dev libssl-dev libsasl2-dev pkg-config

# Get Rust
RUN curl https://sh.rustup.rs -sSf | bash -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

RUN rustup install 1.60.0 && rustup default 1.60.0

RUN sh -c "$(curl -sSfL https://release.solana.com/v1.14.17/install)"
ENV PATH="/root/.local/share/solana/install/active_release/bin:${PATH}"

WORKDIR /geyser

COPY . .

RUN cargo b --release

CMD ./scripts/run.sh