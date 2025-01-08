# T-Invest API Rust SDK

Версия API [1.27](https://github.com/RussianInvestments/investAPI/releases/tag/v1.27)

[Документация для разработчиков](https://developer.tbank.ru/invest/intro/intro)

# Пример
## Cargo.toml
```toml
[dependencies]
t-invest-sdk = "0.2.0"
tokio = { version = "1.42.0", features = ["full"] }
flume = "0.11.1"
anyhow = "1.0.95"
```

## main.rs
```rust
use anyhow::anyhow;
use std::env;
use t_invest_sdk::api::{
    get_candles_request::CandleSource, market_data_request, CandleInstrument,
    FindInstrumentRequest, InstrumentType, MarketDataRequest, SubscribeCandlesRequest,
    SubscriptionAction, SubscriptionInterval,
};
use t_invest_sdk::TInvestSdk;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let token = env::var("API_TOKEN")?;
    let sdk = TInvestSdk::new(&token).await?;
    let mut instruments_service_client = sdk.instruments();
    let mut market_data_stream_service_client = sdk.market_data_stream();

    let find_instrument_response = instruments_service_client
        .find_instrument(FindInstrumentRequest {
            query: "Т-Технологии".to_string(),
            instrument_kind: Some(InstrumentType::Share as i32),
            api_trade_available_flag: Some(true),
        })
        .await?
        .into_inner();

    let instrument = find_instrument_response
        .instruments
        .first()
        .ok_or(anyhow!("Can't find instrument"))?;

    println!("Instrument: {:?}", instrument);

    let (tx, rx) = flume::unbounded();
    let request = MarketDataRequest {
        payload: Some(market_data_request::Payload::SubscribeCandlesRequest(
            SubscribeCandlesRequest {
                subscription_action: SubscriptionAction::Subscribe as i32,
                instruments: vec![CandleInstrument {
                    figi: "".to_string(),
                    interval: SubscriptionInterval::OneMinute as i32,
                    instrument_id: instrument.uid.clone(),
                }],
                waiting_close: false,
                candle_source_type: Some(CandleSource::Unspecified as i32),
            },
        )),
    };
    tx.send(request)?;

    let response = market_data_stream_service_client
        .market_data_stream(rx.into_stream())
        .await?;

    let mut streaming = response.into_inner();

    loop {
        if let Some(next_message) = streaming.message().await? {
            println!("Candle: {:?}", next_message);
        }
    }
}
```
