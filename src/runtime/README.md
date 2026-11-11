# Networking runtime

This directory is reserved for the Tokio client and server. The first prototype will use persistent TCP connections, handle partial I/O, and enforce frame limits and request deadlines. TLS and QUIC are future transport work. Keep transport I/O separate from binary encoding and decoding.
