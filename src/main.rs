// server/src/main.rs
use axum::{
    extract::State,
    response::{sse::Event, Sse},
    routing::get,
    Router,
};
use futures::stream::{self, Stream};
use rumqttc::{AsyncClient, MqttOptions, QoS, Event as MqttEvent, Packet};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RfidReading {
    pub antenna_id: String,
    pub timestamp: String,
    pub rfid_tag: String,
    pub signal_strength: i32,
    pub read_count: u32,
    pub location: String,
}

type ReadingsQueue = Arc<RwLock<Vec<RfidReading>>>;

#[derive(Clone)]
struct AppState {
    readings_queue: ReadingsQueue,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let readings_queue = Arc::new(RwLock::new(Vec::new()));
    let state = AppState {
        readings_queue: Arc::clone(&readings_queue),
    };

    // Iniciar suscriptor MQTT
    start_mqtt_subscriber(Arc::clone(&readings_queue)).await?;

    // Crear router
    let app = Router::new()
        .route("/", get(|| async { "RFID Server SSE - OK" }))
        .route("/api/rfid/stream", get(sse_handler))
        .layer(
            CorsLayer::new()
                .allow_origin(tower_http::cors::Any)
                .allow_methods(tower_http::cors::Any)
                .allow_headers(tower_http::cors::Any)
        )
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:50054").await?;
    println!("🌐 SSE Server listening on http://0.0.0.0:50054");
    println!("   📡 Stream endpoint: http://0.0.0.0:50054/api/rfid/stream");

    axum::serve(listener, app).await?;
    Ok(())
}

async fn sse_handler(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    println!("✅ New SSE client connected");

    let stream = stream::unfold(state, |state| async move {
        loop {
            tokio::time::sleep(Duration::from_millis(10)).await;

            let readings = {
                let mut queue = state.readings_queue.write().await;
                if queue.is_empty() {
                    continue;
                }
                queue.drain(..).collect::<Vec<_>>()
            };

            if !readings.is_empty() {
                for reading in readings {
                    if let Ok(json) = serde_json::to_string(&reading) {
                        let event = Event::default().data(json);
                        return Some((Ok(event), state.clone()));
                    }
                }
            }
        }
    });

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(1))
            .text("keep-alive"),
    )
}

async fn start_mqtt_subscriber(
    readings_queue: ReadingsQueue,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut mqttoptions = MqttOptions::new("rfid_mqtt_sse", "143.198.17.195", 1883);
    mqttoptions.set_keep_alive(Duration::from_secs(5));
    mqttoptions.set_credentials("usuario", "password123");

    let (client, mut eventloop) = AsyncClient::new(mqttoptions, 100);
    client.subscribe("rfid/readings/#", QoS::AtLeastOnce).await?;

    tokio::spawn(async move {
        println!("📡 MQTT subscriber started");

        loop {
            match eventloop.poll().await {
                Ok(notification) => {
                    if let MqttEvent::Incoming(Packet::Publish(publish)) = notification {
                        if let Ok(reading) = serde_json::from_slice::<RfidReading>(&publish.payload) {
                            println!("📥 Antenna: {} | Tag: {}", reading.antenna_id, reading.rfid_tag);

                            let mut queue = readings_queue.write().await;
                            queue.push(reading);

                            // Limitar cola
                            if queue.len() > 10000 {
                                queue.drain(0..5000);
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("MQTT error: {:?}", e);
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }
    });

    Ok(())
}
