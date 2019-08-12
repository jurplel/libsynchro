use std::env;

use cbindgen::{Builder, Config, EnumConfig, Language, RenameRule};

fn main() {
    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();

    let config_enum_c = EnumConfig {
        rename_variants: Some(RenameRule::ScreamingSnakeCase),
        prefix_with_name: true,
        ..EnumConfig::default()
    };

    let config_c = Config {
        language: Language::C,
        enumeration: config_enum_c,
        ..Config::default()
    };

    let config_cxx = Config {
        language: Language::Cxx,
        ..Config::default()
    };

    Builder::new()
        .with_crate(&crate_dir)
        .with_config(config_c)
        .with_parse_deps(true)
        .generate()
        .expect("Unable to generate bindings")
        .write_to_file("target/include/libsynchro.h");

    Builder::new()
        .with_crate(&crate_dir)
        .with_config(config_cxx)
        .with_parse_deps(true)
        .generate()
        .expect("Unable to generate bindings")
        .write_to_file("target/include/libsynchro.hpp");
}
