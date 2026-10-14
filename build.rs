fn main() {
	// rust-embed reads these at compile time, so their changes must rebuild the binary
	println!("cargo:rerun-if-changed=web/build");
	println!("cargo:rerun-if-changed=tracker/dist");
}
