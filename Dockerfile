FROM rust:latest AS builder

WORKDIR /usr/src/omnitea3

COPY . .

RUN cargo build --release

FROM rust:latest

RUN apt-get update && \
    apt-get install -y texlive-base texlive-binaries texlive-latex-extra imagemagick vim

COPY policy.xml /etc/ImageMagick-6/policy.xml

WORKDIR /usr/src/omnitea3

COPY --from=builder /usr/src/omnitea3/target/release/omnitea3 /usr/src/omnitea3/omnitea3

CMD ["./omnitea3"]