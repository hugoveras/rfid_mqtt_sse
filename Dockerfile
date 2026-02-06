FROM rust:1.91.1 as builder

RUN apt update && apt upgrade -y




RUN apt-get update && \
    apt-get install -yq tzdata && \
    ln -fs /usr/share/zoneinfo/America/Puerto_Rico /etc/localtime && \
    dpkg-reconfigure -f noninteractive tzdata


RUN rustup component add rustfmt

 WORKDIR /usr/src/app

# # Copy dependency files first for better caching
 COPY Cargo.toml Cargo.lock  ./

# # Copy source code
 COPY src ./src

# # Build the application in release mode
RUN cargo build --release

# Configures the startup!
WORKDIR /usr/src/app/sources

ENV DATABASE_URL=postgresql://postgres:veras@123@35.196.254.182:5432/doctor_setup
ENV DB_RETRY_ATTEMPTS=3
ENV DB_RETRY_DELAY_MS=3000

EXPOSE 50054
RUN ls /usr/src/app/target/release/
CMD   /usr/src/app/target/release/rfid_mqtt_sse
