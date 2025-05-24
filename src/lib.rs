use api::{
    instruments_service_client::InstrumentsServiceClient,
    market_data_service_client::MarketDataServiceClient,
    market_data_stream_service_client::MarketDataStreamServiceClient,
    operations_service_client::OperationsServiceClient,
    operations_stream_service_client::OperationsStreamServiceClient,
    orders_service_client::OrdersServiceClient,
    orders_stream_service_client::OrdersStreamServiceClient,
    sandbox_service_client::SandboxServiceClient, signal_service_client::SignalServiceClient,
    stop_orders_service_client::StopOrdersServiceClient, users_service_client::UsersServiceClient,
};
use api::{MoneyValue, Quotation};
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use thiserror::Error;
use tonic::transport::ClientTlsConfig;
use tonic::{
    service::{interceptor::InterceptedService, Interceptor},
    transport::Channel,
};

pub mod api;
#[path = "google.api.rs"]
pub mod google_api;

/// Interceptor for T-Invest API requests.
///
/// This struct implements the `Interceptor` trait from tonic to add
/// necessary headers to each API request, including:
/// - Authentication using the provided token
/// - Request tracking ID
/// - Application name
#[derive(Debug, Clone)]
pub struct TInvestInterceptor {
    pub token: String,
}

/// Errors that can occur when interacting with the T-Invest API.
///
/// This enum represents the possible error types that can occur:
/// - `Transport`: Errors related to network connectivity or transport layer
/// - `Status`: Errors returned by the API service itself
#[derive(Error, Debug)]
pub enum TInvestError {
    #[error(transparent)]
    Transport(#[from] tonic::transport::Error),
    #[error(transparent)]
    Status(#[from] tonic::Status),
}

/// Represents the environment to connect to for the T-Invest API.
///
/// There are two possible environments:
/// - `Production`: The live production environment with real accounts and data
/// - `Sandbox`: A testing environment that simulates the production API
#[derive(Debug, Clone, Copy)]
pub enum Environment {
    Production,
    Sandbox,
}

impl Environment {
    /// Returns the base URL for the API based on the selected environment.
    ///
    /// # Returns
    /// A static string containing the complete base URL for API requests.
    fn api_url(&self) -> &'static str {
        match self {
            Environment::Production => "https://invest-public-api.tinkoff.ru:443/",
            Environment::Sandbox => "https://sandbox-invest-public-api.tinkoff.ru:443/",
        }
    }
}

impl Interceptor for TInvestInterceptor {
    /// Intercepts each request to add necessary headers before it's sent to the API.
    ///
    /// This implementation adds the following headers to each request:
    /// - `authorization`: Bearer token for authentication
    /// - `x-tracking-id`: A unique UUID for request tracking
    /// - `x-app-name`: The application identifier
    ///
    /// # Arguments
    /// * `request` - The original request to be modified
    ///
    /// # Returns
    /// The modified request with added headers or an error status
    fn call(
        &mut self,
        mut request: tonic::Request<()>,
    ) -> Result<tonic::Request<()>, tonic::Status> {
        request.metadata_mut().append(
            "authorization",
            format!("bearer {}", self.token)
                .parse()
                .map_err(|_| tonic::Status::internal("Invalid authorization header"))?,
        );

        request.metadata_mut().append(
            "x-tracking-id",
            uuid::Uuid::new_v4()
                .to_string()
                .parse()
                .map_err(|_| tonic::Status::internal("Invalid x-tracking-id"))?,
        );

        request.metadata_mut().append(
            "x-app-name",
            "artemevsevev.t-invest-sdk"
                .parse()
                .map_err(|_| tonic::Status::internal("Invalid x-app-name"))?,
        );

        Ok(request)
    }
}

/// Main SDK client for interacting with the T-Invest API.
///
/// This struct holds channel and interceptor
#[derive(Clone)]
pub struct TInvestSdk {
    channel: Channel,
    interceptor: TInvestInterceptor,
}

impl TInvestSdk {
    /// Creates a new SDK instance connected to the production environment.
    ///
    /// # Arguments
    /// * `token` - API token for authentication
    ///
    /// # Returns
    /// A Result containing either the initialized SDK or an error
    pub async fn new_production(token: &str) -> Result<Self, TInvestError> {
        Self::new(token, Environment::Production).await
    }

    /// Creates a new SDK instance connected to the sandbox (testing) environment.
    ///
    /// # Arguments
    /// * `token` - API token for authentication
    ///
    /// # Returns
    /// A Result containing either the initialized SDK or an error
    pub async fn new_sandbox(token: &str) -> Result<Self, TInvestError> {
        Self::new(token, Environment::Sandbox).await
    }

    /// Creates a new SDK instance with specified token and environment.
    ///
    /// This is the internal constructor used by the convenience methods
    /// `new_production` and `new_sandbox`. It establishes a secure channel
    /// to the T-Invest API using TLS and configures the authentication
    /// interceptor with the provided token.
    ///
    /// # Arguments
    /// * `token` - API token for authentication
    /// * `environment` - The environment to connect to (Production or Sandbox)
    ///
    /// # Returns
    /// A Result containing either the initialized SDK or a TInvestError
    ///
    /// # Errors
    /// Returns an error if:
    /// - TLS configuration fails
    /// - Channel connection cannot be established
    pub async fn new(token: &str, environment: Environment) -> Result<Self, TInvestError> {
        let tls = ClientTlsConfig::new().with_native_roots();
        let channel = Channel::from_static(environment.api_url())
            .tls_config(tls)?
            .connect()
            .await?;
        let interceptor = TInvestInterceptor {
            token: String::from(token),
        };

        Ok(Self {
            channel,
            interceptor,
        })
    }

    /// Returns a client for the Instruments service.
    ///
    /// This service provides methods for working with financial instruments,
    /// including stocks, bonds, ETFs, currencies, and futures.
    pub fn instruments(
        &self,
    ) -> InstrumentsServiceClient<InterceptedService<Channel, TInvestInterceptor>> {
        InstrumentsServiceClient::with_interceptor(self.channel.clone(), self.interceptor.clone())
    }

    /// Returns a client for the Market Data service.
    ///
    /// This service provides methods for requesting market data such as
    /// candles, orderbooks, and last prices.
    pub fn market_data(
        &self,
    ) -> MarketDataServiceClient<InterceptedService<Channel, TInvestInterceptor>> {
        MarketDataServiceClient::with_interceptor(self.channel.clone(), self.interceptor.clone())
    }

    /// Returns a client for the Market Data Stream service.
    ///
    /// This service provides streaming access to real-time market data
    /// including candles, orderbooks, and trades.
    pub fn market_data_stream(
        &self,
    ) -> MarketDataStreamServiceClient<InterceptedService<Channel, TInvestInterceptor>> {
        MarketDataStreamServiceClient::with_interceptor(
            self.channel.clone(),
            self.interceptor.clone(),
        )
    }

    /// Returns a client for the Operations service.
    ///
    /// This service provides methods for working with account operations
    /// such as getting operation history and operation details.
    pub fn operations(
        &self,
    ) -> OperationsServiceClient<InterceptedService<Channel, TInvestInterceptor>> {
        OperationsServiceClient::with_interceptor(self.channel.clone(), self.interceptor.clone())
    }

    /// Returns a client for the Operations Stream service.
    ///
    /// This service provides streaming access to account operations in real-time.
    pub fn operations_stream(
        &self,
    ) -> OperationsStreamServiceClient<InterceptedService<Channel, TInvestInterceptor>> {
        OperationsStreamServiceClient::with_interceptor(
            self.channel.clone(),
            self.interceptor.clone(),
        )
    }

    /// Returns a client for the Orders service.
    ///
    /// This service provides methods for placing, canceling, and getting information
    /// about orders.
    pub fn orders(&self) -> OrdersServiceClient<InterceptedService<Channel, TInvestInterceptor>> {
        OrdersServiceClient::with_interceptor(self.channel.clone(), self.interceptor.clone())
    }

    /// Returns a client for the Orders Stream service.
    ///
    /// This service provides streaming access to order status updates in real-time.
    pub fn orders_stream(
        &self,
    ) -> OrdersStreamServiceClient<InterceptedService<Channel, TInvestInterceptor>> {
        OrdersStreamServiceClient::with_interceptor(self.channel.clone(), self.interceptor.clone())
    }

    /// Returns a client for the Sandbox service.
    ///
    /// This service provides methods for working with the sandbox (test) environment,
    /// including creating and removing sandbox accounts.
    pub fn sandbox(&self) -> SandboxServiceClient<InterceptedService<Channel, TInvestInterceptor>> {
        SandboxServiceClient::with_interceptor(self.channel.clone(), self.interceptor.clone())
    }

    /// Returns a client for the Signal service.
    ///
    /// This service provides methods for working with investment signals and recommendations.
    pub fn signal(&self) -> SignalServiceClient<InterceptedService<Channel, TInvestInterceptor>> {
        SignalServiceClient::with_interceptor(self.channel.clone(), self.interceptor.clone())
    }

    /// Returns a client for the Stop Orders service.
    ///
    /// This service provides methods for placing, canceling, and getting information
    /// about stop orders.
    pub fn stop_orders(
        &self,
    ) -> StopOrdersServiceClient<InterceptedService<Channel, TInvestInterceptor>> {
        StopOrdersServiceClient::with_interceptor(self.channel.clone(), self.interceptor.clone())
    }

    /// Returns a client for the Users service.
    ///
    /// This service provides methods for getting information about user accounts
    /// and their details.
    pub fn users(&self) -> UsersServiceClient<InterceptedService<Channel, TInvestInterceptor>> {
        UsersServiceClient::with_interceptor(self.channel.clone(), self.interceptor.clone())
    }
}

/// Converts a Quotation to a Decimal.
///
/// The Quotation type represents a number as an integer part (units) and a fractional part (nano).
/// This implementation combines them into a single Decimal value.
impl From<Quotation> for Decimal {
    fn from(quotation: Quotation) -> Self {
        Decimal::new(quotation.units, 0) + Decimal::new(quotation.nano as i64, 9).normalize()
    }
}

/// Converts a MoneyValue to a Decimal.
///
/// The MoneyValue type represents a monetary amount as an integer part (units) and a fractional part (nano).
/// This implementation combines them into a single Decimal value, ignoring the currency field.
impl From<MoneyValue> for Decimal {
    fn from(money_value: MoneyValue) -> Self {
        Decimal::new(money_value.units, 0) + Decimal::new(money_value.nano as i64, 9).normalize()
    }
}

/// Attempts to convert a Decimal to a Quotation.
///
/// This implementation separates a Decimal value into integer units and nano parts
/// to create a Quotation. It will return an error if the conversion is not possible.
impl TryFrom<Decimal> for Quotation {
    type Error = String;

    fn try_from(value: Decimal) -> Result<Self, Self::Error> {
        let units = value
            .trunc()
            .to_i64()
            .ok_or_else(|| format!("Can't convert decimal {} to quotation", value))?;
        let nano = (value.fract() * Decimal::new(1_000_000_000, 0))
            .to_i32()
            .ok_or_else(|| format!("Can't convert decimal {} to quotation", value))?;

        Ok(Quotation { units, nano })
    }
}

/// Converts a NaiveDate to a prost_types::Timestamp.
///
/// This function converts a date to a timestamp representing midnight UTC of that date.
/// The timestamp will have seconds set to the Unix timestamp for midnight of the given date,
/// and nanoseconds set to 0.
pub fn naive_date_to_timestamp(date: NaiveDate) -> prost_types::Timestamp {
    let datetime = date.and_hms_opt(0, 0, 0).unwrap();
    let utc_datetime: DateTime<Utc> = DateTime::from_naive_utc_and_offset(datetime, Utc);
    let seconds = utc_datetime.timestamp();

    prost_types::Timestamp { seconds, nanos: 0 }
}

/// Attempts to convert a prost_types::Timestamp to a NaiveDate.
///
/// This function extracts the date part from a timestamp, ignoring the time component.
/// It will return an error if the timestamp represents an invalid date.
pub fn timestamp_to_naive_date(timestamp: &prost_types::Timestamp) -> Result<NaiveDate, String> {
    let datetime = DateTime::<Utc>::from_timestamp(timestamp.seconds, timestamp.nanos as u32)
        .ok_or_else(|| {
            format!(
                "Invalid timestamp: {} seconds, {} nanos",
                timestamp.seconds, timestamp.nanos
            )
        })?;
    Ok(datetime.date_naive())
}

/// Converts a DateTime<Utc> to a prost_types::Timestamp.
///
/// This function converts a UTC datetime to a protobuf timestamp,
/// preserving both seconds and nanoseconds precision.
pub fn datetime_utc_to_timestamp(datetime: DateTime<Utc>) -> prost_types::Timestamp {
    let seconds = datetime.timestamp();
    let nanos = datetime.timestamp_subsec_nanos() as i32;
    
    prost_types::Timestamp { seconds, nanos }
}

/// Attempts to convert a prost_types::Timestamp to a DateTime<Utc>.
///
/// This function converts a protobuf timestamp to a UTC datetime,
/// preserving both seconds and nanoseconds precision.
/// Returns an error if the timestamp represents an invalid datetime.
pub fn timestamp_to_datetime_utc(timestamp: &prost_types::Timestamp) -> Result<DateTime<Utc>, String> {
    DateTime::<Utc>::from_timestamp(timestamp.seconds, timestamp.nanos as u32)
        .ok_or_else(|| {
            format!(
                "Invalid timestamp: {} seconds, {} nanos",
                timestamp.seconds, timestamp.nanos
            )
        })
}

#[cfg(test)]
mod tests {
    use rust_decimal_macros::dec;

    use super::*;

    #[test]
    fn quotation_to_decimal() {
        assert_eq!(dec!(0), Quotation { units: 0, nano: 0 }.into());

        assert_eq!(
            dec!(100),
            Quotation {
                units: 100,
                nano: 0
            }
            .into()
        );

        assert_eq!(
            dec!(-100),
            Quotation {
                units: -100,
                nano: 0
            }
            .into()
        );

        assert_eq!(
            dec!(114.25),
            Quotation {
                units: 114,
                nano: 250000000
            }
            .into()
        );

        assert_eq!(
            dec!(-200.20),
            Quotation {
                units: -200,
                nano: -200000000
            }
            .into()
        );

        assert_eq!(
            dec!(-0.01),
            Quotation {
                units: -0,
                nano: -10000000
            }
            .into()
        );

        assert_eq!(
            dec!(999.999999999),
            Quotation {
                units: 999,
                nano: 999999999
            }
            .into()
        );

        assert_eq!(
            dec!(-999.999999999),
            Quotation {
                units: -999,
                nano: -999999999
            }
            .into()
        );
    }

    #[test]
    fn money_value_to_decimal() {
        assert_eq!(
            dec!(0),
            MoneyValue {
                units: 0,
                nano: 0,
                currency: "".to_string()
            }
            .into()
        );

        assert_eq!(
            dec!(100),
            MoneyValue {
                units: 100,
                nano: 0,
                currency: "".to_string()
            }
            .into()
        );

        assert_eq!(
            dec!(-100),
            MoneyValue {
                units: -100,
                nano: 0,
                currency: "".to_string()
            }
            .into()
        );

        assert_eq!(
            dec!(114.25),
            MoneyValue {
                units: 114,
                nano: 250000000,
                currency: "".to_string()
            }
            .into()
        );

        assert_eq!(
            dec!(-200.20),
            MoneyValue {
                units: -200,
                nano: -200000000,
                currency: "".to_string()
            }
            .into()
        );

        assert_eq!(
            dec!(-0.01),
            MoneyValue {
                units: -0,
                nano: -10000000,
                currency: "".to_string()
            }
            .into()
        );

        assert_eq!(
            dec!(999.999999999),
            MoneyValue {
                units: 999,
                nano: 999999999,
                currency: "".to_string()
            }
            .into()
        );

        assert_eq!(
            dec!(-999.999999999),
            MoneyValue {
                units: -999,
                nano: -999999999,
                currency: "".to_string()
            }
            .into()
        );
    }

    #[test]
    fn decimal_to_quotation() {
        assert_eq!(Ok(Quotation { units: 0, nano: 0 }), dec!(0).try_into());

        assert_eq!(
            Ok(Quotation {
                units: 114,
                nano: 250000000,
            }),
            dec!(114.25).try_into()
        );

        assert_eq!(
            Ok(Quotation {
                units: -200,
                nano: -200000000,
            }),
            dec!(-200.20).try_into()
        );

        assert_eq!(
            Ok(Quotation {
                units: -0,
                nano: -10000000,
            }),
            dec!(-0.01).try_into()
        );

        assert_eq!(
            Ok(Quotation {
                units: 999,
                nano: 999999999,
            }),
            dec!(999.999999999).try_into()
        );

        assert_eq!(
            Ok(Quotation {
                units: -999,
                nano: -999999999,
            }),
            dec!(-999.999999999).try_into()
        );
    }

    #[test]
    fn naive_date_to_timestamp_conversion() {
        // Test Unix epoch date (1970-01-01)
        let epoch_date = NaiveDate::from_ymd_opt(1970, 1, 1).unwrap();
        let timestamp = naive_date_to_timestamp(epoch_date);
        assert_eq!(timestamp.seconds, 0);
        assert_eq!(timestamp.nanos, 0);

        // Test a specific date (2023-12-25)
        let christmas_2023 = NaiveDate::from_ymd_opt(2023, 12, 25).unwrap();
        let timestamp = naive_date_to_timestamp(christmas_2023);
        // 2023-12-25 00:00:00 UTC is 1703462400 seconds since Unix epoch
        assert_eq!(timestamp.seconds, 1703462400);
        assert_eq!(timestamp.nanos, 0);

        // Test a date before epoch (1969-12-31)
        let pre_epoch_date = NaiveDate::from_ymd_opt(1969, 12, 31).unwrap();
        let timestamp = naive_date_to_timestamp(pre_epoch_date);
        assert_eq!(timestamp.seconds, -86400); // -1 day in seconds
        assert_eq!(timestamp.nanos, 0);

        // Test leap year date (2024-02-29)
        let leap_day = NaiveDate::from_ymd_opt(2024, 2, 29).unwrap();
        let timestamp = naive_date_to_timestamp(leap_day);
        // 2024-02-29 00:00:00 UTC is 1709164800 seconds since Unix epoch
        assert_eq!(timestamp.seconds, 1709164800);
        assert_eq!(timestamp.nanos, 0);
    }

    #[test]
    fn timestamp_to_naive_date_conversion() {
        // Test Unix epoch timestamp
        let epoch_timestamp = prost_types::Timestamp {
            seconds: 0,
            nanos: 0,
        };
        let date = timestamp_to_naive_date(&epoch_timestamp).unwrap();
        assert_eq!(date, NaiveDate::from_ymd_opt(1970, 1, 1).unwrap());

        // Test timestamp with time component (should extract only date part)
        let timestamp_with_time = prost_types::Timestamp {
            seconds: 1703462400 + 3661, // Christmas 2023 at 01:01:01
            nanos: 500000000,           // 0.5 seconds
        };
        let date = timestamp_to_naive_date(&timestamp_with_time).unwrap();
        assert_eq!(date, NaiveDate::from_ymd_opt(2023, 12, 25).unwrap());

        // Test negative timestamp (before epoch)
        let pre_epoch_timestamp = prost_types::Timestamp {
            seconds: -86400,
            nanos: 0,
        };
        let date = timestamp_to_naive_date(&pre_epoch_timestamp).unwrap();
        assert_eq!(date, NaiveDate::from_ymd_opt(1969, 12, 31).unwrap());

        // Test leap year timestamp
        let leap_day_timestamp = prost_types::Timestamp {
            seconds: 1709164800,
            nanos: 0,
        };
        let date = timestamp_to_naive_date(&leap_day_timestamp).unwrap();
        assert_eq!(date, NaiveDate::from_ymd_opt(2024, 2, 29).unwrap());
    }

    #[test]
    fn timestamp_to_naive_date_error_cases() {
        // Test invalid timestamp (too far in the future to be represented)
        let invalid_timestamp = prost_types::Timestamp {
            seconds: i64::MAX,
            nanos: 0,
        };
        let result = timestamp_to_naive_date(&invalid_timestamp);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid timestamp"));

        // Test invalid nanoseconds (too large)
        let invalid_nanos = prost_types::Timestamp {
            seconds: 0,
            nanos: 2_000_000_000, // More than 1 second worth of nanoseconds
        };
        let result = timestamp_to_naive_date(&invalid_nanos);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid timestamp"));
    }

    #[test]
    fn round_trip_date_conversion() {
        // Test that converting date -> timestamp -> date gives the same result
        let original_dates = vec![
            NaiveDate::from_ymd_opt(1970, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2023, 12, 25).unwrap(),
            NaiveDate::from_ymd_opt(1969, 12, 31).unwrap(),
            NaiveDate::from_ymd_opt(2024, 2, 29).unwrap(),
            NaiveDate::from_ymd_opt(2000, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(1999, 12, 31).unwrap(),
        ];

        for original_date in original_dates {
            let timestamp = naive_date_to_timestamp(original_date);
            let converted_back = timestamp_to_naive_date(&timestamp).unwrap();
            assert_eq!(original_date, converted_back);
        }
    }

    #[test]
    fn timestamp_conversion_preserves_date_ignores_time() {
        // Test that timestamps with different times on the same date convert to the same date
        let base_date = NaiveDate::from_ymd_opt(2023, 6, 15).unwrap();
        let base_timestamp_seconds = naive_date_to_timestamp(base_date).seconds;

        let timestamps_same_day = vec![
            prost_types::Timestamp {
                seconds: base_timestamp_seconds,
                nanos: 0,
            },
            prost_types::Timestamp {
                seconds: base_timestamp_seconds + 3600, // +1 hour
                nanos: 0,
            },
            prost_types::Timestamp {
                seconds: base_timestamp_seconds + 86399, // 23:59:59 same day
                nanos: 999999999,
            },
        ];

        for timestamp in timestamps_same_day {
            let converted_date = timestamp_to_naive_date(&timestamp).unwrap();
            assert_eq!(converted_date, base_date);
        }
    }

    #[test]
    fn datetime_utc_to_timestamp_conversion() {
        // Test basic conversion
        let datetime = DateTime::from_timestamp(1672531200, 0).unwrap(); // 2023-01-01 00:00:00 UTC
        let timestamp = datetime_utc_to_timestamp(datetime);
        assert_eq!(timestamp.seconds, 1672531200);
        assert_eq!(timestamp.nanos, 0);

        // Test with nanoseconds
        let datetime = DateTime::from_timestamp(1672531200, 123456789).unwrap();
        let timestamp = datetime_utc_to_timestamp(datetime);
        assert_eq!(timestamp.seconds, 1672531200);
        assert_eq!(timestamp.nanos, 123456789);

        // Test edge cases
        let datetime = DateTime::from_timestamp(0, 0).unwrap(); // Unix epoch
        let timestamp = datetime_utc_to_timestamp(datetime);
        assert_eq!(timestamp.seconds, 0);
        assert_eq!(timestamp.nanos, 0);

        // Test negative timestamp (before Unix epoch)
        let datetime = DateTime::from_timestamp(-86400, 0).unwrap(); // 1969-12-31 00:00:00 UTC
        let timestamp = datetime_utc_to_timestamp(datetime);
        assert_eq!(timestamp.seconds, -86400);
        assert_eq!(timestamp.nanos, 0);
    }

    #[test]
    fn timestamp_to_datetime_utc_conversion() {
        // Test basic conversion
        let timestamp = prost_types::Timestamp {
            seconds: 1672531200,
            nanos: 0,
        };
        let datetime = timestamp_to_datetime_utc(&timestamp).unwrap();
        assert_eq!(datetime.timestamp(), 1672531200);
        assert_eq!(datetime.timestamp_subsec_nanos(), 0);

        // Test with nanoseconds
        let timestamp = prost_types::Timestamp {
            seconds: 1672531200,
            nanos: 123456789,
        };
        let datetime = timestamp_to_datetime_utc(&timestamp).unwrap();
        assert_eq!(datetime.timestamp(), 1672531200);
        assert_eq!(datetime.timestamp_subsec_nanos(), 123456789);

        // Test Unix epoch
        let timestamp = prost_types::Timestamp {
            seconds: 0,
            nanos: 0,
        };
        let datetime = timestamp_to_datetime_utc(&timestamp).unwrap();
        assert_eq!(datetime.timestamp(), 0);
        assert_eq!(datetime.timestamp_subsec_nanos(), 0);

        // Test negative timestamp (before Unix epoch)
        let timestamp = prost_types::Timestamp {
            seconds: -86400,
            nanos: 0,
        };
        let datetime = timestamp_to_datetime_utc(&timestamp).unwrap();
        assert_eq!(datetime.timestamp(), -86400);
        assert_eq!(datetime.timestamp_subsec_nanos(), 0);
    }

    #[test]
    fn timestamp_to_datetime_utc_error_cases() {
        // Test invalid timestamp (too far in the future)
        let invalid_timestamp = prost_types::Timestamp {
            seconds: i64::MAX,
            nanos: 0,
        };
        let result = timestamp_to_datetime_utc(&invalid_timestamp);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid timestamp"));

        // Test invalid nanoseconds (too large)
        let invalid_nanos = prost_types::Timestamp {
            seconds: 0,
            nanos: 2_000_000_000, // More than 1 second worth of nanoseconds
        };
        let result = timestamp_to_datetime_utc(&invalid_nanos);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid timestamp"));

        // Test negative nanoseconds
        let negative_nanos = prost_types::Timestamp {
            seconds: 0,
            nanos: -1,
        };
        let result = timestamp_to_datetime_utc(&negative_nanos);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid timestamp"));
    }

    #[test]
    fn round_trip_datetime_utc_conversion() {
        // Test that converting datetime -> timestamp -> datetime gives the same result
        let test_datetimes = vec![
            DateTime::from_timestamp(0, 0).unwrap(), // Unix epoch
            DateTime::from_timestamp(1672531200, 0).unwrap(), // 2023-01-01 00:00:00 UTC
            DateTime::from_timestamp(1672531200, 123456789).unwrap(), // With nanoseconds
            DateTime::from_timestamp(-86400, 0).unwrap(), // Before Unix epoch
            DateTime::from_timestamp(1703980800, 999999999).unwrap(), // 2023-12-31 00:00:00.999999999 UTC
        ];

        for original_datetime in test_datetimes {
            let timestamp = datetime_utc_to_timestamp(original_datetime);
            let converted_back = timestamp_to_datetime_utc(&timestamp).unwrap();
            assert_eq!(original_datetime, converted_back);
        }
    }

    #[test]
    fn datetime_utc_preserves_precision() {
        // Test that conversion preserves nanosecond precision
        let datetime = DateTime::from_timestamp(1672531200, 123456789).unwrap();
        let timestamp = datetime_utc_to_timestamp(datetime);
        
        assert_eq!(timestamp.seconds, 1672531200);
        assert_eq!(timestamp.nanos, 123456789);
        
        let converted_back = timestamp_to_datetime_utc(&timestamp).unwrap();
        assert_eq!(converted_back.timestamp(), 1672531200);
        assert_eq!(converted_back.timestamp_subsec_nanos(), 123456789);
        assert_eq!(datetime, converted_back);
    }

    #[test]
    fn datetime_utc_conversion_different_times() {
        // Test various times throughout a day
        let base_timestamp = 1672531200; // 2023-01-01 00:00:00 UTC
        
        let test_times = vec![
            (base_timestamp, 0), // Midnight
            (base_timestamp + 3600, 0), // 1 AM
            (base_timestamp + 43200, 500000000), // Noon with 0.5 seconds
            (base_timestamp + 82800, 999999999), // 11 PM with max nanoseconds
        ];

        for (seconds, nanos) in test_times {
            let original_datetime = DateTime::from_timestamp(seconds, nanos).unwrap();
            let timestamp = datetime_utc_to_timestamp(original_datetime);
            let converted_back = timestamp_to_datetime_utc(&timestamp).unwrap();
            
            assert_eq!(original_datetime, converted_back);
            assert_eq!(timestamp.seconds, seconds);
            assert_eq!(timestamp.nanos, nanos as i32);
        }
    }
}
