use morphrpc::protocol::supports_version;

#[test]
fn recognizes_only_the_initial_protocol_version() {
    let supported: Vec<u8> = (u8::MIN..=u8::MAX)
        .filter(|&version| supports_version(version))
        .collect();

    assert_eq!(supported, [1]);
}
