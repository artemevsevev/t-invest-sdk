fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_prost_build::configure()
        .build_server(false)
        .out_dir("../src")
        .compile_protos(
            &[
                "../investAPI/src/docs/contracts/instruments.proto",
                "../investAPI/src/docs/contracts/marketdata.proto",
                "../investAPI/src/docs/contracts/operations.proto",
                "../investAPI/src/docs/contracts/orders.proto",
                "../investAPI/src/docs/contracts/sandbox.proto",
                "../investAPI/src/docs/contracts/signals.proto",
                "../investAPI/src/docs/contracts/stoporders.proto",
                "../investAPI/src/docs/contracts/users.proto",
            ],
            &["../investAPI/src/docs/contracts/"],
        )?;

    std::fs::rename(
        "../src/tinkoff.public.invest.api.contract.v1.rs",
        "../src/api.rs",
    )?;

    Ok(())
}
