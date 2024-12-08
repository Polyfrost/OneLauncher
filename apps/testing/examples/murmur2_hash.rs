use onelauncher::utils::crypto;

const CONTENT: &str = "
this is my
text content
";

fn main() {
	let hash = crypto::murmur2(CONTENT.as_bytes());

	dbg!(hash);
	dbg!(hash as u64);
	assert_eq!(hash, 661_461_369);
}