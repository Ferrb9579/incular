use incular_devtools::session::generate_token;

#[test]
fn generated_tokens_are_hex_encoded_and_unique() {
    let first = generate_token().expect("the test platform provides OS entropy");
    let second = generate_token().expect("the test platform provides OS entropy");

    assert_eq!(first.len(), 32);
    assert!(first.bytes().all(|byte| byte.is_ascii_hexdigit()));
    assert_ne!(first, second);
}
