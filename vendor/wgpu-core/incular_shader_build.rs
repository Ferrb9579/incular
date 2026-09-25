use std::{env, fs, path::PathBuf};

pub fn build() {
    println!("cargo:rerun-if-changed=src/indirect_validation");
    println!("cargo:rerun-if-changed=src/timestamp_normalization");
    let output = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let sources = [
        ("draw", fs::read_to_string("src/indirect_validation/validate_draw.wgsl").unwrap()),
        ("dispatch", fs::read_to_string("src/indirect_validation/dispatch_validation.wgsl").unwrap()),
        ("timestamp", format!("{}\n{}",
            fs::read_to_string("src/timestamp_normalization/common.wgsl").unwrap(),
            fs::read_to_string("src/timestamp_normalization/timestamp_normalization.wgsl").unwrap())),
    ];
    for (name, source) in sources {
        let module = naga::front::wgsl::parse_str(&source)
            .unwrap_or_else(|error| panic!("{name}: {}", error.emit_to_string(&source)));
        // These modules use device-dependent capabilities. Their original
        // runtime validator remains authoritative, before HAL compilation.
        fs::write(output.join(format!("{name}.naga")), postcard::to_allocvec(&module).unwrap()).unwrap();
    }
}
