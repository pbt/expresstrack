use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use prost::{DecodeError, Message};
use serde::Serialize;

use anyhow::Result;

use gtfs_rt::{FeedEntity, FeedMessage, TripUpdate};

use reqwest::Client;

use bytes::Bytes;
use tokio::time::sleep;

use std::{env, future::IntoFuture, sync::OnceLock};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

fn api_key() -> &'static str {
    static API_KEY: OnceLock<String> = OnceLock::new();
    API_KEY.get_or_init(|| env::var("API_KEY").unwrap())
}

fn decode_gtfs_rt(bytes: Bytes) -> Result<FeedMessage, DecodeError> {
    let message = FeedMessage::decode(bytes)?;
    Ok(message)
}

fn trip_updates_for(entities: &Vec<FeedEntity>, stations: Vec<String>) -> Result<Vec<TripUpdate>> {
    let mut trips = Vec::new();
    for entity in entities {
        match &entity.trip_update {
            Some(t) => {
                for update in &t.stop_time_update {
                    if let Some(stop_id) = &update.stop_id {
                        if stations.contains(stop_id) {
                            trips.push(t.clone())
                        }
                    }
                }
                // println!("{:#?}", t)
            }
            None => {}
        }
    }

    Ok(trips)
}

#[derive(Serialize)]
struct User {
    id: i64,
    username: String,
}

async fn update_gtfs_feeds(db: Arc<Mutex<Feeds>>) -> Result<(), anyhow::Error> {
    loop {
        println!("Reloading...");
        match update_gtfs_feeds_inner(&db).await {
            Ok(()) => {}
            Err(e) => {
                println!("Errored! {}", e)
            }
        }
        sleep(Duration::from_secs(15)).await;
    }

    Ok(())
}

async fn update_gtfs_feeds_inner(db: &Arc<Mutex<Feeds>>) -> Result<(), anyhow::Error> {
    let gtfs_client = Client::new();
    let gtfs_feed = gtfs_client
        .get("https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/nyct%2Fgtfs-g")
        .header("x-api-key", api_key())
        .send()
        .await?;
    let bytes = gtfs_feed.bytes().await?;
    let feed_msg = decode_gtfs_rt(bytes)?;
    db.lock().unwrap().g = Some(feed_msg);
    Ok(())
}

#[derive(Default)]
struct Feeds {
    g: Option<FeedMessage>,
}

#[tokio::main]
async fn main() {
    // initialize shared state
    let db = Arc::new(Mutex::new(Feeds::default()));

    // build our application with a route
    let app = Router::new()
        .route("/", get(handler))
        .with_state(db.clone());

    // run it
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    println!("listening on {}", listener.local_addr().unwrap());

    let update_gtfs_feeds_task = tokio::task::spawn(update_gtfs_feeds(db));
    let web_task = tokio::task::spawn(axum::serve(listener, app).into_future());

    // TODO: how will i handle errors?
    let _ = tokio::join!(update_gtfs_feeds_task, web_task);
}

async fn handler(
    State(state): State<Arc<Mutex<Feeds>>>,
) -> Result<Json<Vec<TripUpdate>>, AppError> {
    let state = state.lock().unwrap();
    let g = state.g.as_ref().unwrap();

    let arrivals = trip_updates_for(&g.entity, vec![String::from("A42N"), String::from("A42S")]);
    Ok(Json(arrivals?))
}

// Make our own error that wraps `anyhow::Error`.
struct AppError(anyhow::Error);

// Tell axum how to convert `AppError` into a response.
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Something went wrong: {}", self.0),
        )
            .into_response()
    }
}

// This enables using `?` on functions that return `Result<_, anyhow::Error>` to turn them into
// `Result<_, AppError>`. That way you don't need to do that manually.
impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}
