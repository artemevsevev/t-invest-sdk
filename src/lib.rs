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
}
