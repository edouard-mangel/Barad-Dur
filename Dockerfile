# --- Build stage: static Rust binary via musl ---
FROM rust:1.94-alpine AS builder

RUN apk add --no-cache musl-dev openssl-dev openssl-libs-static pkgconf cmake make perl git

WORKDIR /build
COPY Cargo.toml Cargo.lock ./

COPY src/ src/
RUN OPENSSL_STATIC=1 cargo build --release --target x86_64-unknown-linux-musl

# --- Git stage: grab statically-linked git from alpine ---
FROM alpine:3.21 AS git-env

RUN apk add --no-cache git

# Collect git and all its shared libs for the scratch image
RUN mkdir -p /git-dist/usr/bin /git-dist/usr/libexec /git-dist/lib /git-dist/etc/ssl && \
    cp /usr/bin/git /git-dist/usr/bin/ && \
    cp -r /usr/libexec/git-core /git-dist/usr/libexec/ && \
    # Copy musl dynamic linker + shared libs needed by git
    ldd /usr/bin/git | awk '/=>/ {print $3}' | xargs -I{} cp {} /git-dist/lib/ && \
    cp /lib/ld-musl-x86_64.so.1 /git-dist/lib/ && \
    # CA certificates for HTTPS clones
    cp -r /etc/ssl/certs /git-dist/etc/ssl/

# --- Git config: trust all mounted directories ---
FROM alpine:3.21 AS gitconfig
RUN echo -e "[safe]\n\tdirectory = *" > /etc/gitconfig

# --- Final stage: scratch ---
FROM scratch

# CA certificates
COPY --from=git-env /git-dist/etc/ssl/certs /etc/ssl/certs
ENV SSL_CERT_DIR=/etc/ssl/certs

# Git global config (trust mounted repos)
COPY --from=gitconfig /etc/gitconfig /etc/gitconfig

# Musl dynamic linker + shared libs (for git)
COPY --from=git-env /git-dist/lib /lib

# Git binary + helpers
COPY --from=git-env /git-dist/usr/bin/git /usr/bin/git
COPY --from=git-env /git-dist/usr/libexec/git-core /usr/libexec/git-core

# Statically-linked barad-dur binary
COPY --from=builder /build/target/x86_64-unknown-linux-musl/release/barad-dur /usr/local/bin/barad-dur

# /tmp is needed for remote repo cloning (tempfile crate)
WORKDIR /repo
ENTRYPOINT ["/usr/local/bin/barad-dur"]
CMD ["analyze", "."]
