fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_files = [
        "proto/acl.proto",
        "proto/capability.proto",
        "proto/directory.proto",
        "proto/receipt.proto",
    ];
    for path in proto_files {
        println!("cargo:rerun-if-changed={path}");
    }

    prost_build::Config::new()
        .bytes(["."])
        .compile_protos(&proto_files, &["proto/"])?;
    Ok(())
}
