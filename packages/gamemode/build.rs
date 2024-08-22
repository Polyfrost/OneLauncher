fn main() {
	if std::env::var_os("CARGO_CFG_LINUX").is_some() {
		cc::Build::new()
			.include("include")
			.file("stub/stub.c")
			.opt_level(3)
			.debug(false)
			.warnings(true)
			.compile("stub");

		println!("cargo:rerun-if-changed=stub.c");
	} else {
		println!("cargo:rerun-if-changed=build.rs");
	}
}
