# syntax=docker/dockerfile:1
FROM rustembedded/cross:x86_64-unknown-linux-gnu-0.2.1

RUN dpkg --add-architecture amd64 && apt update && apt install --assume-yes libxkbcommon-dev libssl-dev openssl libportaudio2 libasound2 libasound2-dev wget curl pkg-config 

ENV RUST_BACKTRACE=full
ENV PORTAUDIO_ONLY_STATIC=1
