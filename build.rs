fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::compile_protos("proto/health/v1/health.proto")?;
    Ok(())
}
