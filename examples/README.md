# get_user example

Start the server in one terminal and the client in another.

```sh
cargo run --example get_user -- server
cargo run --example get_user -- client
```

The client sends two requests on one connection and prints `user 1: Ada` and `user 2: Grace`. The server listens on `127.0.0.1:8080`. Stop it with Ctrl-C.

Operation `1` accepts an eight-byte big-endian user ID. A successful response contains the eight-byte ID followed by a UTF-8 name. Error payloads contain UTF-8 text. The example uses an in-memory lookup and binds to loopback for local testing.
