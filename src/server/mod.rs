use axum::{
	extract::Path, http::StatusCode, response::Redirect, routing::get, Extension, Json, Router,
};
use serde_json::json;
use std::net::SocketAddr;
use teloxide::{
	types::{MessageId, User},
	Bot,
};
use tokio::signal;

use crate::{
	bot::{on_verified, JoinRequests},
	config::AppConfig,
};

pub async fn start(bot: Bot, config: AppConfig, bot_data: User, join_requests: JoinRequests) {
	let app = Router::new()
		.route(
			"/",
			get(|| async {
				Redirect::permanent(&format!("https://t.me/{}", bot_data.username.unwrap()))
			}),
		)
		.route("/verify/:msg_id", get(verify_page).post(verify_api))
		.layer(Extension(bot))
		.layer(Extension(config))
		.layer(Extension(join_requests));

	let addr = SocketAddr::from(([127, 0, 0, 1], 8000));
	log::info!("Starting server at http://{addr}");

	axum::Server::bind(&addr)
		.serve(app.into_make_service())
		.with_graceful_shutdown(async move { signal::ctrl_c().await.unwrap() })
		.await
		.unwrap();
}

async fn verify_page(
	Path(msg_id): Path<MessageId>,
	Extension(join_reqs): Extension<JoinRequests>,
) -> Result<&'static str, StatusCode> {
	let _join_req = join_reqs.get(&msg_id).ok_or(StatusCode::NOT_FOUND)?;

	//TODO: WorldID proof generation

	Ok("Hello, World!")
}

#[derive(Debug, serde::Deserialize)]
struct VerifyRequest {
	proof: String,
	merkle_root: String,
	nullifier_hash: String,
	credential_type: String,
}

async fn verify_api(
	Extension(bot): Extension<Bot>,
	Path(msg_id): Path<MessageId>,
	Extension(config): Extension<AppConfig>,
	Extension(join_reqs): Extension<JoinRequests>,
	Json(req): Json<VerifyRequest>,
) -> Result<&'static str, StatusCode> {
	let join_req = join_reqs.get(&msg_id).ok_or(StatusCode::NOT_FOUND)?;

	reqwest::Client::new()
		.post(format!(
			"https://developer.worldcoin.org/api/v1/verify/{}",
			config.app_id
		))
		.json(&json!({
			"signal": msg_id,
			"proof": req.proof,
			"action": join_req.chat_id,
			"merkle_root": req.merkle_root,
			"nullifier_hash": req.nullifier_hash,
			"credential_type": req.credential_type,
		}))
		.send()
		.await
		.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
		.error_for_status()
		.map_err(|_| StatusCode::BAD_REQUEST)?;

	drop(join_req);

	on_verified(bot, msg_id, join_reqs)
		.await
		.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

	Ok("Verified!")
}
