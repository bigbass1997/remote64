# syntax=docker/dockerfile:1
FROM rustembedded/cross:x86_64-unknown-linux-gnu-0.2.1

RUN dpkg --add-architecture amd64 && apt update && apt install --assume-yes libxkbcommon-dev libssl-dev openssl
