use oneclient_content::packages::curseforge_fingerprint;

#[test]
fn curseforge_fingerprint_ignores_padding_whitespace() {
    let a = curseforge_fingerprint(b"hello");
    let b = curseforge_fingerprint(b"\t\n\r hello \n\t\r");
    assert_eq!(a, b);
}

#[test]
fn curseforge_fingerprint_differs_for_content() {
    let a = curseforge_fingerprint(b"hello");
    let b = curseforge_fingerprint(b"world");
    assert_ne!(a, b);
}
