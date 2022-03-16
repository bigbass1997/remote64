[![License: BSD 2-Clause](https://img.shields.io/badge/License-BSD%202--Clause-blue)](LICENSE) [![CERN License](https://img.shields.io/badge/license-CERN%20OHL--W--V2-blue)](license/cern_ohl_w_v2.txt)
### Description
This project enables N64 homebrew developers to test their roms on real hardware, without actually owning said hardware.

Project is split in two parts: the server which handles all incoming requests, performs them on hardware, and sends back
what happened; and the client which is what developers use to test their roms.

The server side can be run by anyone that has a compatible hardware setup. It manages incomming requests for testing,
captures the console's output, and relays that information back to the client. Servers may have a varying set of
capabilities. If the client requests a feature the server doesn't support, the user will be notified.

If a client connects while another test is in progress, the new client will be placed in a queue and automatically
serviced once the current test has finished. The maximum length of a test is defined by the server, but will likely
be quite generous.

### Why does this exist?
Ultimately, this is an attempt to reduce the cost of entry into N64 homebrew and research. Especially given the chip
shortage and other circumstances that have severely limited flashcart production.

While it is possible to ask others to test a rom build, it's also possible that no one will be available when needed.
This is _not_ intended to completely replace developers purchasing their own flashcarts/consoles, nor to replace community
testers; rather it is here to supplement those testing methods.

### Which server should I connect to?
**TODO**

### Server Capabilities
The bare minimum a server setup requires is some method to automatically upload and start the provided ROM image, and a
capture device to record the video output with. The server software will not work without a valid video stream, even if
live playback isn't enabled.

Optional capabilities include:
- Live playback (requires decent upload speed)
- Audio recording (for final recording, and live playback if enabled)
- Controller input (requires live playback and input passthrough)

### Repo Structure
`/client/`, `/common/`, and `/server/` make up the software side, while `/controller/` contains the hardware used by the
server for powering the system on/off, and passing in controller inputs.

`/docker/` contains container build script(s) that can be used for cross-compiling.

### Compiling/Building
If you wish to build from source, for your own system, Rust is integrated with the `cargo` build system. To install Rust and `cargo`, just follow [these instructions](https://doc.rust-lang.org/cargo/getting-started/installation.html). Once installed, while in the project's root directory, run `cargo build --bin remote64-client --release` to build (use `--bin remote64-server` for server builds), or use `cargo run --bin remote64-client --release` to run directly. The built binary will be available in `./target/release/`

To cross-compile builds for other operating systems, you can use [rust-embedded/cross](https://github.com/rust-embedded/cross).

The `Cross.toml` file is configured to expect a local docker container for linux and windows builds.

##### Linux
Docker: `docker build -t remote64-image-linux:tag docker/linux/`  
Rust: `cross build --target x86_64-unknown-linux-gnu --bin remote64-client --release`

##### Windows
Docker: `docker build -t remote64-image-windows:tag docker/windows/`  
Rust: `cross build --target x86_64-pc-windows-gnu --bin remote64-client --release`  
_Note: Cross-compiling for windows is currently broken. I cannot get the container to recognize the portaudio library._