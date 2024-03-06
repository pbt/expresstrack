use axum::{
    debug_handler,
    extract::{Query, State},
    http::StatusCode,
    response::Json,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use prost::{DecodeError, Message};
use serde::Deserialize;
use serde::Serialize;

use anyhow::Result;

use gtfs_rt::{FeedEntity, FeedMessage, TripUpdate, VehicleDescriptor, VehiclePosition};

use reqwest::Client;

use bytes::Bytes;
use tokio::time::sleep;

use std::{collections::HashMap, env, future::IntoFuture, sync::OnceLock};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

fn api_key() -> &'static str {
    static API_KEY: OnceLock<String> = OnceLock::new();
    API_KEY.get_or_init(|| env::var("API_KEY").unwrap())
}

fn decode_gtfs_rt(bytes: Bytes) -> anyhow::Result<FeedMessage> {
    let message = FeedMessage::decode(bytes)?;
    Ok(message)
}

fn vehicle_pos_for(
    entities: &Vec<FeedEntity>, /*stations: Vec<String>*/
) -> Result<Vec<VehiclePosition>> {
    let mut vehicles = Vec::new();
    for entity in entities {
        match &entity.vehicle {
            Some(v) => {
                vehicles.push(v.clone())
                // println!("{:#?}", t)
            }
            None => {}
        }
    }

    Ok(vehicles)
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

async fn update_gtfs_feeds(db: Arc<Mutex<Feeds>>, endpoint: String) -> Result<(), anyhow::Error> {
    loop {
        match update_gtfs_feeds_inner(&db, &endpoint).await {
            Ok(msg) => {
                db.lock().unwrap().feeds.insert(endpoint.clone(), Some(msg));
            }
            Err(e) => {
                println!("Errored! {}", e)
            }
        }
        sleep(Duration::from_secs(15)).await;
    }

    Ok(())
}

async fn update_gtfs_feeds_inner(
    db: &Arc<Mutex<Feeds>>,
    endpoint: &String,
) -> Result<FeedMessage, anyhow::Error> {
    let gtfs_client = Client::new();
    let gtfs_feed = gtfs_client
        .get(endpoint)
        .header("x-api-key", api_key())
        .send()
        .await?;
    let bytes = gtfs_feed.bytes().await?;
    let feed_msg = decode_gtfs_rt(bytes)?;
    Ok(feed_msg)
}

#[derive(Default)]
struct Feeds {
    feeds: HashMap<String, Option<FeedMessage>>,
}

impl Feeds {
    pub fn new(feeds: HashMap<String, Option<FeedMessage>>) -> Self {
        Self { feeds }
    }
}

#[tokio::main]
async fn main() {
    // initialize shared state
    let db = Arc::new(Mutex::new(Feeds::new(HashMap::from([
        (
            "https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/nyct%2Fgtfs-g".to_string(),
            None,
        ),
        (
            "https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/nyct%2Fgtfs-ace".to_string(),
            None,
        ),
        (
            "https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/nyct%2Fgtfs-bdfm".to_string(),
            None,
        ),
        (
            "https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/nyct%2Fgtfs-jz".to_string(),
            None,
        ),
        (
            "https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/nyct%2Fgtfs-nqrw".to_string(),
            None,
        ),
        (
            "https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/nyct%2Fgtfs-l".to_string(),
            None,
        ),
        (
            "https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/nyct%2Fgtfs".to_string(),
            None,
        ),
        (
            "https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/nyct%2Fgtfs-si".to_string(),
            None,
        ),
    ]))));

    // build our application with a route
    let app = Router::new()
        .route("/api/v1/vehicles/", get(get_vehicle_positions))
        .route("/api/v1/trains_for_station/", get(get_trains_for_station))
        .with_state(db.clone());

    // run it
    let listener = tokio::net::TcpListener::bind("127.0.0.1:4567")
        .await
        .unwrap();
    println!("listening on {}", listener.local_addr().unwrap());

    let update_gtfs_feeds_task_g = tokio::task::spawn(update_gtfs_feeds(
        db.clone(),
        "https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/nyct%2Fgtfs-g".to_string(),
    ));
    let update_gtfs_feeds_task_ace = tokio::task::spawn(update_gtfs_feeds(
        db.clone(),
        "https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/nyct%2Fgtfs-ace".to_string(),
    ));
    let update_gtfs_feeds_task_bdfm = tokio::task::spawn(update_gtfs_feeds(
        db.clone(),
        "https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/nyct%2Fgtfs-bdfm".to_string(),
    ));
    let update_gtfs_feeds_task_jz = tokio::task::spawn(update_gtfs_feeds(
        db.clone(),
        "https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/nyct%2Fgtfs-jz".to_string(),
    ));
    let update_gtfs_feeds_task_nqrw = tokio::task::spawn(update_gtfs_feeds(
        db.clone(),
        "https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/nyct%2Fgtfs-nqrw".to_string(),
    ));
    let update_gtfs_feeds_task_l = tokio::task::spawn(update_gtfs_feeds(
        db.clone(),
        "https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/nyct%2Fgtfs-l".to_string(),
    ));
    let update_gtfs_feeds_task_1234567 = tokio::task::spawn(update_gtfs_feeds(
        db.clone(),
        "https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/nyct%2Fgtfs".to_string(),
    ));
    let update_gtfs_feeds_task_sir = tokio::task::spawn(update_gtfs_feeds(
        db.clone(),
        "https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/nyct%2Fgtfs-si".to_string(),
    ));
    let web_task = tokio::task::spawn(axum::serve(listener, app).into_future());

    // TODO: how will i handle errors?
    let _ = tokio::join!(
        update_gtfs_feeds_task_g,
        update_gtfs_feeds_task_ace,
        update_gtfs_feeds_task_bdfm,
        update_gtfs_feeds_task_jz,
        update_gtfs_feeds_task_nqrw,
        update_gtfs_feeds_task_l,
        update_gtfs_feeds_task_1234567,
        update_gtfs_feeds_task_sir,
        web_task
    );
}

#[derive(Default, Deserialize)]
struct StationsQuery {
    stations: String,
}

async fn get_trains_for_station(
    query: Option<Query<StationsQuery>>,
    State(state): State<Arc<Mutex<Feeds>>>,
) -> Result<Json<Vec<TripUpdate>>, AppError> {
    let state = state.lock().unwrap();
    let Query(query) = query.unwrap_or_default();

    let feeds = state.feeds.values();
    let mut entity: Vec<FeedEntity> = vec![];

    for feed in feeds {
        match feed {
            Some(f) => entity.append(&mut f.entity.clone()),
            None => {}
        }
    }

    let arrivals = trip_updates_for(
        &entity,
        query.stations.split(",").map(|i| i.to_string()).collect(), // vec!["A42N".to_string(), "A42S".to_string()],
    );
    Ok(Json(arrivals?))
}

#[debug_handler]
async fn get_vehicle_positions(
    State(state): State<Arc<Mutex<Feeds>>>,
) -> Result<Json<Vec<VehiclePosition>>, AppError> {
    let state = state.lock().unwrap();

    let feeds = state.feeds.values();

    let mut entity: Vec<FeedEntity> = vec![];
    for feed in feeds {
        match feed {
            Some(f) => entity.append(&mut f.entity.clone()),
            None => {}
        }
    }

    Ok(Json(vehicle_pos_for(&entity)?))
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
