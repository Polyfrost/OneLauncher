use std::path::Path;

use actix_web::{get, web::Data, HttpResponse, Responder};

#[get("/")]
pub async fn index() -> impl Responder {
	HttpResponse::Ok().body("Polyfrost API")
}

#[get("/oneconfig/{version}-{loader}")]
pub async fn oneconfig(
	data: Data<state::AppState>,
	path: Path<(String, String)>,
) -> impl Responder {
}
