use axum::{middleware, routing::{get}, Json, Router};
use serde::{Deserialize};
use String;
use axum::extract::Query;
use axum::extract::Path;
use axum::http::{Method, Uri};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get_service;
use serde_json::json;
use tokio::net::TcpListener;
use tower_cookies::CookieManagerLayer;
use tower_http::services::ServeDir;
use uuid::Uuid;
use crate::ctx::Ctx;
use crate::error::Error;
use crate::error::Result;
use crate::log::log_request;
use crate::model::ModelController;

mod error;
mod web;
mod model;
mod ctx;
mod log;


#[derive(Debug, Deserialize)]
struct HelloParams {
    name: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let mc = ModelController::new().await?;

    let routes_apis = web::routes_tickets::routes(mc.clone())
        .route_layer(middleware::from_fn(web::mw_auth::mw_require_auth));

    let app = Router::new()
        .merge(router_hello())
        .merge(web::routes_login::routes())
        .nest("/api", routes_apis)
        .layer(middleware::map_response(main_reponse_mapper))
        .layer(middleware::from_fn_with_state(
            mc.clone(),
            web::mw_auth::mw_ctx_resolver,
        ))
        .layer(CookieManagerLayer::new())
        .fallback_service(router_static());


    let listener = TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();

    Ok(())
}

async fn main_reponse_mapper(
    ctx: Option<Ctx>,
    uri: Uri,
    req_method: Method,
    res: Response
) -> Response {
    println!("->> {:<12} - main_response_mapper", "RES_MAPPER");

    let uuid = Uuid::new_v4();
    let service_error = res.extensions().get::<Error>();
    let client_status_error = service_error.map(|se| se.client_status_and_error());

    let error_response = client_status_error.as_ref().map(|(status_code, client_error)| {
        let client_error_body = json!({
            "error": client_error.as_ref(),
            "req_uuid": uuid.to_string(),
        });

        println!("->> client_error_body: {client_error_body}");
        (*status_code, Json(client_error_body)).into_response()
    });

    // println!("->> server log line - {uuid} - Error: {service_error:?}");
    let client_error = client_status_error.unzip().1;
    let _ = log_request(uuid, req_method, uri, ctx, service_error, client_error).await;

    println!();
    error_response.unwrap_or(res)
}

fn router_hello() -> Router {
    Router::new().route("/hello", get(handle_hello))
        .route("/hello2/:name", get(handle_hello2))
}

fn router_static() -> Router {
    Router::new().nest_service("/", get_service(ServeDir::new("./")))
}

async fn handle_hello(Query(params): Query<HelloParams>) -> impl IntoResponse {
    println!("->> {:<12} - handler_hello - {params:?}", "HANDLER");
    let name = params.name.as_deref().unwrap_or("World");
    Html(format!("<h1>Hello, {name}!</h1>"))
}

async fn handle_hello2(Path(name): Path<String>) -> impl IntoResponse {
    println!("->> {:<12} - handler_hello2 - {name:?}", "HANDLER");
    Html(format!("<h1>Hello, {name}!</h1>"))
}
