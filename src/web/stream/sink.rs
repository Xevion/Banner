//! Send half of a stream WebSocket connection.

use axum::extract::ws::{Message, WebSocket};
use futures::SinkExt;
use futures::stream::SplitSink;

use crate::telemetry::WS_MESSAGES;
use crate::web::stream::protocol::{StreamErrorCode, StreamServerMessage};

/// The server-to-client side of a stream connection.
///
/// Owns the split sink and counts every outbound message. Each send reports whether the
/// connection is still alive; `false` means the socket is gone and the caller should stop.
pub struct StreamSink {
    sink: SplitSink<WebSocket, Message>,
}

impl StreamSink {
    pub const fn new(sink: SplitSink<WebSocket, Message>) -> Self {
        Self { sink }
    }

    pub async fn send(&mut self, message: &StreamServerMessage) -> bool {
        let kind = message.kind_label();

        // Serializing our own message cannot fail for any input the client controls, so this keeps the
        // connection rather than tearing it down; counting it is what stops it being silent.
        let Ok(json) = serde_json::to_string(message) else {
            metrics::counter!(WS_MESSAGES, "direction" => "out", "kind" => kind, "outcome" => "encode_failed")
                .increment(1);
            return true;
        };

        let sent = self.sink.send(Message::Text(json.into())).await.is_ok();
        let outcome = if sent { "sent" } else { "failed" };
        metrics::counter!(WS_MESSAGES, "direction" => "out", "kind" => kind, "outcome" => outcome).increment(1);

        sent
    }

    pub async fn send_error(&mut self, request_id: Option<String>, code: StreamErrorCode, message: &str) -> bool {
        let msg = StreamServerMessage::Error {
            request_id,
            code,
            message: message.to_string(),
        };
        self.send(&msg).await
    }
}
