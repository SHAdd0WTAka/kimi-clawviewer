/// Build script for cv-shared crate.
///
/// Compiles Protocol Buffer definitions from the `proto/` directory
/// into Rust code using the `prost-build` crate.
use std::io;

fn main() -> io::Result<()> {
    let proto_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("proto");

    let protos = [
        proto_dir.join("rendezvous.proto"),
        proto_dir.join("message.proto"),
    ];

    // Verify proto files exist before compilation
    for proto in &protos {
        if !proto.exists() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("Proto file not found: {}", proto.display()),
            ));
        }
    }

    prost_build::compile_protos(&protos, &[proto_dir])?;

    // Re-run build script if proto files change
    for proto in &protos {
        println!("cargo:rerun-if-changed={}", proto.display());
    }

    Ok(())
}
