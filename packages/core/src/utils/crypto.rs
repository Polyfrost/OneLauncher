use std::path::PathBuf;

use whoami::Result;

pub fn sha1(bytes: &[u8]) -> String {
	sha1_smol::Sha1::from(bytes).digest().to_string()
}

pub fn sha1_file(file: &PathBuf) -> Result<String> {
	Ok(sha1(&std::fs::read(file)?))
}

pub fn sha1_files(files: Vec<&PathBuf>) -> Vec<Option<String>> {
	let mut hashes = Vec::new();
	for file in files {
		hashes.push(sha1_file(file).ok());
	}
	hashes
}

const CURSEFORGE_FINGERPRINT_SEED: u32 = 1;
pub fn murmur2(bytes: &[u8]) -> u32 {
	let normalized = normalize_byte_array(bytes);
	murmur2::murmur2(normalized.as_slice(), CURSEFORGE_FINGERPRINT_SEED)
}

pub fn murmur2_file(file: &PathBuf) -> Result<u32> {
	Ok(murmur2(&std::fs::read(file)?))
}

pub fn murmur2_files(files: Vec<&PathBuf>) -> Vec<Option<u32>> {
	let mut hashes = Vec::new();
	for file in files {
		hashes.push(murmur2_file(file).ok());
	}
	hashes
}

// Curseforge and their documented code :D (it wasn't documented)
// https://github.com/CurseForgeCommunity/.NET-APIClient/blob/2c4f5f613d20025f9286fdd53592f8519022918f/Murmur2.cs#L8-L21
// https://github.com/comp500/infra.link/blob/a298502fdb15b8939fe0effe71ed42ad06f3946f/murmur2/go/main.go#L10-L40
fn normalize_byte_array(bytes: &[u8]) -> Vec<u8> {
	let normalized = bytes
		.iter()
		.filter(|byte| !matches!(byte, 9 | 10 | 13 | 32))
		.copied()
		.collect::<Vec<u8>>();
	// println!("Old len {}", bytes.len());
	// println!("New len {}", normalized.len());
	normalized
}
