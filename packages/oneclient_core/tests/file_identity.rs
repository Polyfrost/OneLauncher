use oneclient_core::packages::{curseforge_fingerprint, FileIdentity};

#[test]
fn curseforge_fingerprint_ignores_padding_whitespace() {
	let a = curseforge_fingerprint(b"hello");
	let b = curseforge_fingerprint(b"\t\n\r hello \n\t\r");
	assert_eq!(a, b);
}

#[test]
fn curseforge_fingerprint_empty_is_stable() {
	assert_eq!(curseforge_fingerprint(&[]), curseforge_fingerprint(&[]));
}

#[test]
fn curseforge_fingerprint_differs_for_content() {
	let a = curseforge_fingerprint(b"hello");
	let b = curseforge_fingerprint(b"world");
	assert_ne!(a, b);
}

#[test]
fn file_identity_from_bytes_sets_sha1_and_cf_fingerprint() {
	let id = FileIdentity::from_bytes(b"test jar content");
	assert_eq!(id.sha1.len(), 40);
	assert!(id.cf_fingerprint.is_some());
}

#[test]
fn file_identity_from_sha1_has_no_cf_fingerprint() {
	let id = FileIdentity::from_sha1("a".repeat(40));
	assert!(id.cf_fingerprint.is_none());
}

#[test]
fn file_identity_with_fingerprint_roundtrip() {
	let id = FileIdentity::from_sha1("abc").with_curseforge_fingerprint(12345);
	assert_eq!(id.cf_fingerprint, Some(12345));
}
