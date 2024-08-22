fn main() {
	build_stub();
}

#[cfg(target_os = "linux")]
fn build_stub() {
	cc::Build::new()
		.include("include")
		.file("stub/stub.c")
		.opt_level(3)
		.debug(false)
		.warnings(true)
		.compile("stub");

	println!("cargo:rerun-if-changed=stub.c");
}
