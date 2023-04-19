FROM rust:latest AS builder

WORKDIR /usr/src

# Create blank project
RUN USER=root cargo new omnitea3

# We want dependencies cached, so copy those first.
COPY Cargo.toml /usr/src/omnitea3/

WORKDIR /usr/src/omnitea3

# This is a dummy build to get the dependencies cached.
RUN cargo build --release

RUN rm -rf src
RUN rm -rf target/release/deps/omnitea3*

# Now copy in the rest of the sources
COPY src /usr/src/omnitea3/src/


ARG PROMPT_FILE_VAR
ENV PROMPT_FILE=$PROMPT_FILE_VAR

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