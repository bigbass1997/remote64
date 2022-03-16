# syntax=docker/dockerfile:1
FROM rustembedded/cross:x86_64-pc-windows-gnu-0.2.1

RUN apt-get update && apt-get install wget curl libasound2 libasound2-dev portaudio19-dev pkg-config -y



ENV RUST_BACKTRACE=full
#ENV PORTAUDIO_ONLY_STATIC=1
