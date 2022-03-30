pub mod client;
pub mod connection;
pub mod error;
pub mod lib_new;

use std::pin::Pin;
use std::task::{Context, Poll};
use crate::{
    error::ClientError,
    model::{Candle, MarketData, Trade},
    client::binance::BinanceMessage
};
use barter_integration::socket::{
    Transformer, error::SocketError, ExchangeSocket
};
use async_trait::async_trait;
use futures::{Sink, Stream};
use serde::de::DeserializeOwned;
use serde::Serialize;
use tokio::net::TcpStream;
use tokio_tungstenite::{
    connect_async, MaybeTlsStream, WebSocketStream,
    tungstenite::Message as WsMessage
};
use tokio::sync::mpsc::UnboundedReceiver;
use tracing::debug;
use barter_integration::socket::protocol::websocket::{WebSocket, WebSocketParser};


// Todo: general:
//  - Increase test coverage significantly now you know the PoC design works
//  - Unsure all .unwrap()s have been exchanged for more robust handling
//  - Ensure proper error handling & swapping unwraps() for more robust handling
//     '-> ensure all methods are returning an appropriate Result which is handled by caller

// Todo: connection.rs:
//  - Improve method of confirming subscription request so test_subscribe unit test passed
//     '-> subscription succeeded even if it didn't, need to confirm first message arrives?
//     '-> ensure logging is aligned once this has been done
//  - manage() add in connection fixing, reconnections

/// Client trait defining the behaviour of all implementing ExchangeClients. All methods return
/// a stream of normalised data.
#[async_trait]
pub trait ExchangeClient {
    const EXCHANGE_NAME: &'static str;
    async fn consume_trades(&mut self, symbol: String, ) -> Result<UnboundedReceiver<Trade>, ClientError>;
    async fn consume_candles(&mut self, symbol: String, interval: &str) -> Result<UnboundedReceiver<Candle>, ClientError>;
}

/// Utilised to subscribe to an exchange's [`WebSocketStream`] via a ConnectionHandler (eg/ Trade stream).
pub trait Subscription {
    /// Constructs a new [`Subscription`] implementation.
    fn new(stream_name: String, ticker_pair: String) -> Self;
    /// Serializes the [`Subscription`] in a String data format.
    fn as_text(&self) -> Result<String, ClientError>
    where
        Self: Serialize,
    {
        Ok(serde_json::to_string(self)?)
    }
}

/// Returns a stream identifier that can be used to route messages from a [`Subscription`].
pub trait StreamIdentifier {
    fn get_stream_id(&self) -> Identifier;
}

/// Enum returned from [`StreamIdentifier`] representing if a struct has an identifiable stream Id.
pub enum Identifier {
    Yes(String),
    No,
}

/// Connect asynchronously to an exchange's server, returning a [`WebSocketStream`].
async fn connect(base_uri: &String) -> Result<WSStream, ClientError> {
    debug!("Establishing WebSocket connection to: {:?}", base_uri);
    connect_async(base_uri)
        .await
        .and_then(|(ws_stream, _)| Ok(ws_stream))
        .map_err(|err| ClientError::WebSocketConnect(err))
}

pub mod test_util {
    use crate::model::Candle;
    use chrono::Utc;

    pub fn candle() -> Candle {
        Candle {
            start_timestamp: Utc::now(),
            end_timestamp: Utc::now(),
            open: 1000.0,
            high: 1100.0,
            low: 900.0,
            close: 1050.0,
            volume: 1000000000.0,
            trade_count: 100,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_connect() {
        struct TestCase {
            input_base_uri: String,
            expected_can_connect: bool,
        }

        let test_cases = vec![
            TestCase {
                // Test case 0: Not a valid WS base URI
                input_base_uri: "not a valid base uri".to_string(),
                expected_can_connect: false,
            },
            TestCase {
                // Test case 1: Valid Binance WS base URI
                input_base_uri: "wss://stream.binance.com:9443/ws".to_string(),
                expected_can_connect: true,
            },
            TestCase {
                // Test case 2: Valid Bitstamp WS base URI
                input_base_uri: "wss://ws.bitstamp.net/".to_string(),
                expected_can_connect: true,
            },
        ];

        for (index, test) in test_cases.into_iter().enumerate() {
            let actual_result = connect(&test.input_base_uri).await;
            assert_eq!(
                test.expected_can_connect,
                actual_result.is_ok(),
                "Test case: {:?}",
                index
            );
        }
    }
}