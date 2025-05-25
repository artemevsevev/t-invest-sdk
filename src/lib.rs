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

/// Перехватчик для запросов T-Invest API.
///
/// Эта структура реализует трейт `Interceptor` из tonic для добавления
/// необходимых заголовков к каждому API запросу, включая:
/// - Аутентификацию с использованием предоставленного токена
/// - ID отслеживания запроса
/// - Имя приложения
#[derive(Debug, Clone)]
pub struct TInvestInterceptor {
    pub token: String,
}

/// Ошибки, которые могут возникнуть при взаимодействии с T-Invest API.
///
/// Это перечисление представляет возможные типы ошибок, которые могут возникнуть:
/// - `Transport`: Ошибки, связанные с сетевым подключением или транспортным уровнем
/// - `Status`: Ошибки, возвращаемые самим API сервисом
#[derive(Error, Debug)]
pub enum TInvestError {
    #[error(transparent)]
    Transport(#[from] tonic::transport::Error),
    #[error(transparent)]
    Status(#[from] tonic::Status),
}

/// Представляет среду для подключения к T-Invest API.
///
/// Существует две возможные среды:
/// - `Production`: Живая продакшн среда с реальными счетами и данными
/// - `Sandbox`: Тестовая среда, которая симулирует продакшн API
#[derive(Debug, Clone, Copy)]
pub enum Environment {
    Production,
    Sandbox,
}

impl Environment {
    /// Возвращает базовый URL для API на основе выбранной среды.
    ///
    /// # Возвращает
    /// Статическую строку, содержащую полный базовый URL для API запросов.
    fn api_url(&self) -> &'static str {
        match self {
            Environment::Production => "https://invest-public-api.tinkoff.ru:443/",
            Environment::Sandbox => "https://sandbox-invest-public-api.tinkoff.ru:443/",
        }
    }
}

impl Interceptor for TInvestInterceptor {
    /// Перехватывает каждый запрос для добавления необходимых заголовков перед отправкой в API.
    ///
    /// Эта реализация добавляет следующие заголовки к каждому запросу:
    /// - `authorization`: Bearer токен для аутентификации
    /// - `x-tracking-id`: Уникальный UUID для отслеживания запроса
    /// - `x-app-name`: Идентификатор приложения
    ///
    /// # Аргументы
    /// * `request` - Исходный запрос, который нужно изменить
    ///
    /// # Возвращает
    /// Изменённый запрос с добавленными заголовками или статус ошибки
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

/// Основной SDK клиент для взаимодействия с T-Invest API.
///
/// Эта структура содержит канал и перехватчик
#[derive(Clone)]
pub struct TInvestSdk {
    channel: Channel,
    interceptor: TInvestInterceptor,
}

impl TInvestSdk {
    /// Создаёт новый экземпляр SDK, подключённый к продакшн среде.
    ///
    /// # Аргументы
    /// * `token` - API токен для аутентификации
    ///
    /// # Возвращает
    /// Result, содержащий либо инициализированный SDK, либо ошибку
    pub async fn new_production(token: &str) -> Result<Self, TInvestError> {
        Self::new(token, Environment::Production).await
    }

    /// Создаёт новый экземпляр SDK, подключённый к sandbox (тестовой) среде.
    ///
    /// # Аргументы
    /// * `token` - API токен для аутентификации
    ///
    /// # Возвращает
    /// Result, содержащий либо инициализированный SDK, либо ошибку
    pub async fn new_sandbox(token: &str) -> Result<Self, TInvestError> {
        Self::new(token, Environment::Sandbox).await
    }

    /// Создаёт новый экземпляр SDK с указанным токеном и средой.
    ///
    /// Это внутренний конструктор, используемый удобными методами
    /// `new_production` и `new_sandbox`. Он устанавливает безопасный канал
    /// к T-Invest API, используя TLS, и настраивает перехватчик аутентификации
    /// с предоставленным токеном.
    ///
    /// # Аргументы
    /// * `token` - API токен для аутентификации
    /// * `environment` - Среда для подключения (Production или Sandbox)
    ///
    /// # Возвращает
    /// Result, содержащий либо инициализированный SDK, либо TInvestError
    ///
    /// # Ошибки
    /// Возвращает ошибку, если:
    /// - Не удалось настроить TLS конфигурацию
    /// - Невозможно установить соединение с каналом
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

    /// Возвращает клиент для сервиса Instruments.
    ///
    /// Этот сервис предоставляет методы для работы с финансовыми инструментами,
    /// включая акции, облигации, ETF, валюты и фьючерсы.
    ///
    /// Описание: https://developer.tbank.ru/invest/services/instruments/head-instruments
    ///
    /// Методы: https://developer.tbank.ru/invest/services/instruments/methods
    pub fn instruments(
        &self,
    ) -> InstrumentsServiceClient<InterceptedService<Channel, TInvestInterceptor>> {
        InstrumentsServiceClient::with_interceptor(self.channel.clone(), self.interceptor.clone())
    }

    /// Возвращает клиент для сервиса Market Data.
    ///
    /// Этот сервис предоставляет методы для запроса рыночных данных, таких как
    /// свечи, стаканы и последние цены.
    ///
    /// Описание: https://developer.tbank.ru/invest/services/quotes/head-marketdata
    ///
    /// Методы: https://developer.tbank.ru/invest/services/quotes/marketdata
    pub fn market_data(
        &self,
    ) -> MarketDataServiceClient<InterceptedService<Channel, TInvestInterceptor>> {
        MarketDataServiceClient::with_interceptor(self.channel.clone(), self.interceptor.clone())
    }

    /// Возвращает клиент для сервиса Market Data Stream.
    ///
    /// Этот сервис предоставляет потоковый доступ к рыночным данным в реальном времени,
    /// включая свечи, стаканы и сделки.
    ///
    /// Описание: https://developer.tbank.ru/invest/services/quotes/head-marketdata
    ///
    /// Методы: https://developer.tbank.ru/invest/services/quotes/marketdata
    pub fn market_data_stream(
        &self,
    ) -> MarketDataStreamServiceClient<InterceptedService<Channel, TInvestInterceptor>> {
        MarketDataStreamServiceClient::with_interceptor(
            self.channel.clone(),
            self.interceptor.clone(),
        )
    }

    /// Возвращает клиент для сервиса Operations.
    ///
    /// Этот сервис предоставляет методы для работы с операциями по счёту,
    /// такими как получение истории операций и деталей операций.
    ///
    /// Описание: https://developer.tbank.ru/invest/services/operations/head-operations
    ///
    /// Методы: https://developer.tbank.ru/invest/services/operations/methods
    pub fn operations(
        &self,
    ) -> OperationsServiceClient<InterceptedService<Channel, TInvestInterceptor>> {
        OperationsServiceClient::with_interceptor(self.channel.clone(), self.interceptor.clone())
    }

    /// Возвращает клиент для сервиса Operations Stream.
    ///
    /// Этот сервис предоставляет потоковый доступ к операциям по счёту в реальном времени.
    ///
    /// Описание: https://developer.tbank.ru/invest/services/operations/head-operations
    ///
    /// Методы: https://developer.tbank.ru/invest/services/operations/methods
    pub fn operations_stream(
        &self,
    ) -> OperationsStreamServiceClient<InterceptedService<Channel, TInvestInterceptor>> {
        OperationsStreamServiceClient::with_interceptor(
            self.channel.clone(),
            self.interceptor.clone(),
        )
    }

    /// Возвращает клиент для сервиса Orders.
    ///
    /// Этот сервис предоставляет методы для размещения, отмены и получения информации
    /// о заявках.
    ///
    /// Описание: https://developer.tbank.ru/invest/services/orders/head-orders
    ///
    /// Методы: https://developer.tbank.ru/invest/services/orders/methods
    pub fn orders(&self) -> OrdersServiceClient<InterceptedService<Channel, TInvestInterceptor>> {
        OrdersServiceClient::with_interceptor(self.channel.clone(), self.interceptor.clone())
    }

    /// Возвращает клиент для сервиса Orders Stream.
    ///
    /// Этот сервис предоставляет потоковый доступ к обновлениям статуса заявок в реальном времени.
    ///
    /// Описание: https://developer.tbank.ru/invest/services/orders/head-orders
    ///
    /// Методы: https://developer.tbank.ru/invest/services/orders/methods
    pub fn orders_stream(
        &self,
    ) -> OrdersStreamServiceClient<InterceptedService<Channel, TInvestInterceptor>> {
        OrdersStreamServiceClient::with_interceptor(self.channel.clone(), self.interceptor.clone())
    }

    /// Возвращает клиент для сервиса Sandbox.
    ///
    /// Этот сервис предоставляет методы для работы с sandbox (тестовой) средой,
    /// включая создание и удаление sandbox счетов.
    pub fn sandbox(&self) -> SandboxServiceClient<InterceptedService<Channel, TInvestInterceptor>> {
        SandboxServiceClient::with_interceptor(self.channel.clone(), self.interceptor.clone())
    }

    /// Возвращает клиент для сервиса Signal.
    ///
    /// Этот сервис предоставляет методы для работы с инвестиционными сигналами и рекомендациями.
    ///
    /// Описание: https://developer.tbank.ru/invest/services/signals/head-signals
    ///
    /// Методы: https://developer.tbank.ru/invest/services/signals/methods
    pub fn signal(&self) -> SignalServiceClient<InterceptedService<Channel, TInvestInterceptor>> {
        SignalServiceClient::with_interceptor(self.channel.clone(), self.interceptor.clone())
    }

    /// Возвращает клиент для сервиса Stop Orders.
    ///
    /// Этот сервис предоставляет методы для размещения, отмены и получения информации
    /// о стоп-заявках.
    ///
    /// Описание: https://developer.tbank.ru/invest/services/stop-orders/head-stoporders
    ///
    /// Методы: https://developer.tbank.ru/invest/services/stop-orders/stoporders
    pub fn stop_orders(
        &self,
    ) -> StopOrdersServiceClient<InterceptedService<Channel, TInvestInterceptor>> {
        StopOrdersServiceClient::with_interceptor(self.channel.clone(), self.interceptor.clone())
    }

    /// Возвращает клиент для сервиса Users.
    ///
    /// Этот сервис предоставляет методы для получения информации о пользовательских счетах
    /// и их деталях.
    ///
    /// Описание: https://developer.tbank.ru/invest/services/accounts/head-account
    ///
    /// Методы: https://developer.tbank.ru/invest/services/accounts/users
    pub fn users(&self) -> UsersServiceClient<InterceptedService<Channel, TInvestInterceptor>> {
        UsersServiceClient::with_interceptor(self.channel.clone(), self.interceptor.clone())
    }
}

/// Преобразует Quotation в Decimal.
///
/// Тип Quotation представляет число как целую часть (units) и дробную часть (nano).
/// Эта реализация объединяет их в единое значение Decimal.
impl From<Quotation> for Decimal {
    fn from(quotation: Quotation) -> Self {
        Decimal::new(quotation.units, 0) + Decimal::new(quotation.nano as i64, 9).normalize()
    }
}

/// Преобразует MoneyValue в Decimal.
///
/// Тип MoneyValue представляет денежную сумму как целую часть (units) и дробную часть (nano).
/// Эта реализация объединяет их в единое значение Decimal, игнорируя поле валюты.
impl From<MoneyValue> for Decimal {
    fn from(money_value: MoneyValue) -> Self {
        Decimal::new(money_value.units, 0) + Decimal::new(money_value.nano as i64, 9).normalize()
    }
}

/// Пытается преобразовать Decimal в Quotation.
///
/// Эта реализация разделяет значение Decimal на целые единицы и нано-части
/// для создания Quotation. Возвращает ошибку, если преобразование невозможно.
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
