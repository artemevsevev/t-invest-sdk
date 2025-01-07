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
use thiserror::Error;
use tonic::transport::ClientTlsConfig;
use tonic::{
    service::{interceptor::InterceptedService, Interceptor},
    transport::Channel,
};

pub mod api;
#[path = "google.api.rs"]
pub mod google_api;

#[derive(Debug, Clone)]
pub struct TInvestInterceptor {
    pub token: String,
}

#[derive(Error, Debug)]
pub enum TInvestError {
    #[error(transparent)]
    Transport(#[from] tonic::transport::Error),
    #[error(transparent)]
    Status(#[from] tonic::Status),
}

impl Interceptor for TInvestInterceptor {
    fn call(&mut self, request: tonic::Request<()>) -> Result<tonic::Request<()>, tonic::Status> {
        let mut req = request;
        req.metadata_mut().append(
            "authorization",
            format!("bearer {}", self.token).parse().unwrap(),
        );
        req.metadata_mut().append(
            "x-tracking-id",
            uuid::Uuid::new_v4().to_string().parse().unwrap(),
        );
        req.metadata_mut()
            .append("x-app-name", "artemevsevev.t-invest-sdk".parse().unwrap());

        Ok(req)
    }
}

#[derive(Clone)]
pub struct TInvestSdk {
    instruments_service_client:
        InstrumentsServiceClient<InterceptedService<Channel, TInvestInterceptor>>,
    market_data_service_client:
        MarketDataServiceClient<InterceptedService<Channel, TInvestInterceptor>>,
    market_data_stream_service_client:
        MarketDataStreamServiceClient<InterceptedService<Channel, TInvestInterceptor>>,
    operations_service_client:
        OperationsServiceClient<InterceptedService<Channel, TInvestInterceptor>>,
    operations_stream_service_client:
        OperationsStreamServiceClient<InterceptedService<Channel, TInvestInterceptor>>,
    orders_service_client: OrdersServiceClient<InterceptedService<Channel, TInvestInterceptor>>,
    orders_stream_service_client:
        OrdersStreamServiceClient<InterceptedService<Channel, TInvestInterceptor>>,
    sandbox_service_client: SandboxServiceClient<InterceptedService<Channel, TInvestInterceptor>>,
    signal_service_client: SignalServiceClient<InterceptedService<Channel, TInvestInterceptor>>,
    stop_orders_service_client:
        StopOrdersServiceClient<InterceptedService<Channel, TInvestInterceptor>>,
    users_service_client: UsersServiceClient<InterceptedService<Channel, TInvestInterceptor>>,
}

impl TInvestSdk {
    pub async fn new(token: &str) -> Result<Self, TInvestError> {
        let tls = ClientTlsConfig::new().with_native_roots();
        let channel = Channel::from_static("https://invest-public-api.tinkoff.ru:443/")
            .tls_config(tls)?
            .connect()
            .await?;
        let interceptor = TInvestInterceptor {
            token: String::from(token),
        };

        Ok(Self {
            instruments_service_client: InstrumentsServiceClient::with_interceptor(
                channel.clone(),
                interceptor.clone(),
            ),
            market_data_service_client: MarketDataServiceClient::with_interceptor(
                channel.clone(),
                interceptor.clone(),
            ),
            market_data_stream_service_client: MarketDataStreamServiceClient::with_interceptor(
                channel.clone(),
                interceptor.clone(),
            ),
            operations_service_client: OperationsServiceClient::with_interceptor(
                channel.clone(),
                interceptor.clone(),
            ),
            operations_stream_service_client: OperationsStreamServiceClient::with_interceptor(
                channel.clone(),
                interceptor.clone(),
            ),
            orders_service_client: OrdersServiceClient::with_interceptor(
                channel.clone(),
                interceptor.clone(),
            ),
            orders_stream_service_client: OrdersStreamServiceClient::with_interceptor(
                channel.clone(),
                interceptor.clone(),
            ),
            sandbox_service_client: SandboxServiceClient::with_interceptor(
                channel.clone(),
                interceptor.clone(),
            ),
            signal_service_client: SignalServiceClient::with_interceptor(
                channel.clone(),
                interceptor.clone(),
            ),
            stop_orders_service_client: StopOrdersServiceClient::with_interceptor(
                channel.clone(),
                interceptor.clone(),
            ),
            users_service_client: UsersServiceClient::with_interceptor(
                channel.clone(),
                interceptor.clone(),
            ),
        })
    }

    pub fn instruments(
        &self,
    ) -> InstrumentsServiceClient<InterceptedService<Channel, TInvestInterceptor>> {
        self.instruments_service_client.clone()
    }

    pub fn market_data(
        &self,
    ) -> MarketDataServiceClient<InterceptedService<Channel, TInvestInterceptor>> {
        self.market_data_service_client.clone()
    }

    pub fn market_data_stream(
        &self,
    ) -> MarketDataStreamServiceClient<InterceptedService<Channel, TInvestInterceptor>> {
        self.market_data_stream_service_client.clone()
    }

    pub fn operations(
        &self,
    ) -> OperationsServiceClient<InterceptedService<Channel, TInvestInterceptor>> {
        self.operations_service_client.clone()
    }

    pub fn operations_stream(
        &self,
    ) -> OperationsStreamServiceClient<InterceptedService<Channel, TInvestInterceptor>> {
        self.operations_stream_service_client.clone()
    }

    pub fn orders(&self) -> OrdersServiceClient<InterceptedService<Channel, TInvestInterceptor>> {
        self.orders_service_client.clone()
    }

    pub fn orders_stream(
        &self,
    ) -> OrdersStreamServiceClient<InterceptedService<Channel, TInvestInterceptor>> {
        self.orders_stream_service_client.clone()
    }

    pub fn sandbox(&self) -> SandboxServiceClient<InterceptedService<Channel, TInvestInterceptor>> {
        self.sandbox_service_client.clone()
    }

    pub fn signal(&self) -> SignalServiceClient<InterceptedService<Channel, TInvestInterceptor>> {
        self.signal_service_client.clone()
    }

    pub fn stop_orders(
        &self,
    ) -> StopOrdersServiceClient<InterceptedService<Channel, TInvestInterceptor>> {
        self.stop_orders_service_client.clone()
    }

    pub fn users(&self) -> UsersServiceClient<InterceptedService<Channel, TInvestInterceptor>> {
        self.users_service_client.clone()
    }
}
