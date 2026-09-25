use std::{env, fs, path::PathBuf};

fn main() {
    let source_dir = PathBuf::from("src/shaders");
    println!("cargo:rerun-if-changed={}", source_dir.display());
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo provides OUT_DIR"));
    let mut paths: Vec<_> = fs::read_dir(&source_dir)
        .expect("built-in shader directory")
        .map(|entry| entry.expect("shader directory entry").path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "wgsl")
        })
        .collect();
    paths.sort();
    let mut bindings = String::new();
    for path in paths {
        let source = fs::read_to_string(&path).expect("read built-in shader");
        let module = naga::front::wgsl::parse_str(&source).unwrap_or_else(|error| {
            panic!("{}: {}", path.display(), error.emit_to_string(&source))
        });
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let name = path
            .file_stem()
            .expect("shader stem")
            .to_str()
            .expect("UTF-8 shader name");
        let bytes = postcard::to_allocvec(&module).expect("serialize built-in shader");
        fs::write(output.join(format!("{name}.naga")), bytes).expect("write shader IR");
        bindings.push_str(&format!(
            "pub(crate) const {}: &[u8] = include_bytes!(concat!(env!(\"OUT_DIR\"), \"/{name}.naga\"));\n",
            name.to_ascii_uppercase()
        ));
    }
    fs::write(output.join("built_in_shaders.rs"), bindings).expect("write shader bindings");
}
