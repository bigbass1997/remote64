[![License: BSD 2-Clause](https://img.shields.io/badge/License-BSD%202--Clause-blue)](LICENSE)
### Description
This project enables N64 homebrew developers to test their roms on real hardware, without actually owning said hardware.

There are two programs: the server and the client. The server side can be run by anyone that has a compatible hardware
setup. It manages incomming requests for testing, captures the console's output, and relays that information back to the
client. Servers may have a varying set of capabilities. If the client requests a feature the server doesn't support,
the user will be notified.

If supported by the server, by default, the client will display a live (but delayed) view of the console's video.
The client has the option to request a recording of the test as well, which may also include audio. If another client
attempts to connect during an ongoing test, it will be placed into a queue.

### Why does this exist?
Ultimately, this is an attempt to reduce the cost of entry into N64 homebrew and research.

While it is possible to ask others to test a rom build, it's also possible that no one will be available at that time.
This project enables automatic testing of new builds at any time.

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
