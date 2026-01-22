fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_prost_build::configure()
        .build_server(false)
        .out_dir("../src")
        .compile_protos(
            &[
                "../contracts/instruments.proto",
                "../contracts/marketdata.proto",
                "../contracts/operations.proto",
                "../contracts/orders.proto",
                "../contracts/sandbox.proto",
                "../contracts/signals.proto",
                "../contracts/stoporders.proto",
                "../contracts/users.proto",
            ],
            &["../contracts/"],
        )?;

    std::fs::rename(
        "../src/tinkoff.public.invest.api.contract.v1.rs",
        "../src/api.rs",
    )?;

    Ok(())
}
