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
///
/// # Документация
/// - [Описание](https://developer.tbank.ru/invest/intro/intro)
/// - [Получить токен](https://developer.tbank.ru/invest/intro/intro/token#получить-токен)
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
    /// # Документация:
    ///   - [Описание сервиса](https://developer.tbank.ru/invest/services/instruments/head-instruments)
    ///   - [gRPC-методы](https://developer.tbank.ru/invest/services/instruments/methods)
    ///   - [Глоссарий и дополнительная информация о методах сервиса инструментов](https://developer.tbank.ru/invest/services/instruments/more-instrument)
    ///   - [FAQ](https://developer.tbank.ru/invest/services/instruments/faq_instruments)
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
    /// # Документация:
    /// - [Описание сервиса](https://developer.tbank.ru/invest/services/quotes/head-marketdata)
    /// - [gRPC-методы](https://developer.tbank.ru/invest/services/quotes/marketdata)
    /// - [FAQ](https://developer.tbank.ru/invest/services/quotes/faq_marketdata)
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
    /// # Документация:
    /// - [Описание сервиса](https://developer.tbank.ru/invest/services/quotes/head-marketdata)
    /// - [gRPC-методы](https://developer.tbank.ru/invest/services/quotes/marketdata)
    /// - [FAQ](https://developer.tbank.ru/invest/services/quotes/faq_marketdata)
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
    /// # Документация:
    /// - [Описание сервиса](https://developer.tbank.ru/invest/services/operations/head-operations)
    /// - [Особенности методов сервиса операций](https://developer.tbank.ru/invest/services/operations/operations_problems)
    /// - [gRPC-методы](https://developer.tbank.ru/invest/services/operations/methods)
    /// - [FAQ](https://developer.tbank.ru/invest/services/operations/faq_operations)
    pub fn operations(
        &self,
    ) -> OperationsServiceClient<InterceptedService<Channel, TInvestInterceptor>> {
        OperationsServiceClient::with_interceptor(self.channel.clone(), self.interceptor.clone())
    }

    /// Возвращает клиент для сервиса Operations Stream.
    ///
    /// Этот сервис предоставляет потоковый доступ к операциям по счёту в реальном времени.
    ///
    /// # Документация:
    /// - [Описание сервиса](https://developer.tbank.ru/invest/services/operations/head-operations)
    /// - [Особенности методов сервиса операций](https://developer.tbank.ru/invest/services/operations/operations_problems)
    /// - [gRPC-методы](https://developer.tbank.ru/invest/services/operations/methods)
    /// - [FAQ](https://developer.tbank.ru/invest/services/operations/faq_operations)
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
    /// # Документация:
    /// - [Описание сервиса](https://developer.tbank.ru/invest/services/orders/head-orders)
    /// - [gRPC-методы](https://developer.tbank.ru/invest/services/orders/methods)
    /// - [Асинхронный метод выставления заявок](https://developer.tbank.ru/invest/services/orders/async)
    /// - [FAQ](https://developer.tbank.ru/invest/services/orders/faq_orders)
    pub fn orders(&self) -> OrdersServiceClient<InterceptedService<Channel, TInvestInterceptor>> {
        OrdersServiceClient::with_interceptor(self.channel.clone(), self.interceptor.clone())
    }

    /// Возвращает клиент для сервиса Orders Stream.
    ///
    /// Этот сервис предоставляет потоковый доступ к обновлениям статуса заявок в реальном времени.
    ///
    /// # Документация:
    /// - [Описание сервиса](https://developer.tbank.ru/invest/services/orders/head-orders)
    /// - [gRPC-методы](https://developer.tbank.ru/invest/services/orders/methods)
    /// - [Стрим заявок](https://developer.tbank.ru/invest/services/orders/orders_state_stream)
    /// - [FAQ](https://developer.tbank.ru/invest/services/orders/faq_orders)
    pub fn orders_stream(
        &self,
    ) -> OrdersStreamServiceClient<InterceptedService<Channel, TInvestInterceptor>> {
        OrdersStreamServiceClient::with_interceptor(self.channel.clone(), self.interceptor.clone())
    }

    /// Возвращает клиент для сервиса Sandbox.
    ///
    /// Этот сервис предоставляет методы для работы с sandbox (тестовой) средой,
    /// включая создание и удаление sandbox счетов.
    ///
    /// # Документация:
    /// - [Описание сервиса](https://developer.tbank.ru/invest/intro/developer/sandbox/)
    /// - [gRPC-методы](https://developer.tbank.ru/invest/intro/developer/sandbox/methods)
    /// - [Песочница и prod](https://developer.tbank.ru/invest/intro/developer/sandbox/url_difference)
    /// - [FAQ](https://developer.tbank.ru/invest/intro/developer/sandbox/faq_sandbox)
    pub fn sandbox(&self) -> SandboxServiceClient<InterceptedService<Channel, TInvestInterceptor>> {
        SandboxServiceClient::with_interceptor(self.channel.clone(), self.interceptor.clone())
    }

    /// Возвращает клиент для сервиса Signal.
    ///
    /// Этот сервис предоставляет методы для работы с инвестиционными сигналами и рекомендациями.
    ///
    /// # Документация:
    /// - [Описание сервиса](https://developer.tbank.ru/invest/services/signals/head-signals)
    /// - [gRPC-методы](https://developer.tbank.ru/invest/services/signals/methods)
    pub fn signal(&self) -> SignalServiceClient<InterceptedService<Channel, TInvestInterceptor>> {
        SignalServiceClient::with_interceptor(self.channel.clone(), self.interceptor.clone())
    }

    /// Возвращает клиент для сервиса Stop Orders.
    ///
    /// Этот сервис предоставляет методы для размещения, отмены и получения информации
    /// о стоп-заявках.
    ///
    /// # Документация:
    /// - [Описание сервиса](https://developer.tbank.ru/invest/services/stop-orders/head-stoporders)
    /// - [gRPC-методы](https://developer.tbank.ru/invest/services/stop-orders/stoporders)
    /// - [FAQ](https://developer.tbank.ru/invest/services/stop-orders/faq_stoporders)
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
    /// # Документация:
    /// - [Описание сервиса](https://developer.tbank.ru/invest/services/accounts/head-account)
    /// - [gRPC-методы](https://developer.tbank.ru/invest/services/accounts/users)
    /// - [FAQ](https://developer.tbank.ru/invest/services/accounts/faq_users)
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
