FROM rust:latest AS builder

WORKDIR /usr/src

# Create blank project
RUN USER=root cargo new omnitea3

# We want dependencies cached, so copy those first.
COPY Cargo.toml /usr/src/omnitea3/

WORKDIR /usr/src/omnitea3

# This is a dummy build to get the dependencies cached.
RUN cargo build --release

# Now copy in the rest of the sources
COPY src /usr/src/omnitea3/src/

# This is the actual build.
RUN cargo build --release 

FROM debian:bullseye-slim

RUN apt-get update && \
    apt-get install -y texlive-base texlive-xetex texlive-latex-extra imagemagick vim \
    pandoc

COPY policy.xml /etc/ImageMagick-6/policy.xml

WORKDIR /usr/src/omnitea3

COPY --from=builder /usr/src/omnitea3/target/release/omnitea3 /usr/src/omnitea3/omnitea3

CMD ["./omnitea3"]