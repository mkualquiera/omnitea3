FROM rust:latest AS builder

WORKDIR /usr/src/omnitea3

COPY . .

RUN cargo build --release


FROM alpine:latest

WORKDIR /usr/src/omnitea3

COPY --from=builder /usr/src/omnitea3/target/release/omnitea3 /usr/src/omnitea3/omnitea3

CMD ["./omnitea3"]