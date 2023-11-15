use axum::{
	extract::Path,
	http::StatusCode,
	response::{Html, Redirect},
	routing::get,
	Extension, Json, Router,
};
use indoc::formatdoc;
use serde_json::json;
use std::net::SocketAddr;
use teloxide::{
	types::{ChatId, MessageId, User},
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
		.route(
			"/verify/:chat_id/:msg_id",
			get(verify_page).post(verify_api),
		)
		.layer(Extension(bot))
		.layer(Extension(config))
		.layer(Extension(join_requests));

	let addr = SocketAddr::from(([0, 0, 0, 0], 8000));
	log::info!("Starting server at http://{addr}");

	axum::Server::bind(&addr)
		.serve(app.into_make_service())
		.with_graceful_shutdown(async move { signal::ctrl_c().await.unwrap() })
		.await
		.unwrap();
}

async fn verify_page(
	Path(chat_id): Path<ChatId>,
	Path(msg_id): Path<MessageId>,
	Extension(config): Extension<AppConfig>,
	Extension(join_reqs): Extension<JoinRequests>,
) -> Result<Html<String>, StatusCode> {
	if !join_reqs.contains_key(&(chat_id, msg_id)) {
		return Err(StatusCode::NOT_FOUND);
	}

	let page = formatdoc! {"<!DOCTYPE html>
        <html lang=\"en\">
            <head>
                <meta charset=\"UTF-8\" />
                <meta http-equiv=\"X-UA-Compatible\" content=\"IE=edge\" />
                <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\" />
                <title>Verify with World ID</title>
            </head>
            <body>
                <script src=\"https://unpkg.com/@worldcoin/idkit@0.5.1/build/idkit-js.js\"></script>

                <script>
                    IDKit.init({{
                        signal: '{msg_id}',
                        app_id: '{app_id}',
                        action: '{chat_id}',
                        enableTelemetry: true,
                        credential_types: ['phone', 'orb'],
                    }})

                    window.addEventListener('load', async () => {{
                        await fetch('/verify/{chat_id}/{msg_id}', {{
                            method: 'POST',
                            body: JSON.stringify(await IDKit.open()),
                            headers: {{ 'Content-Type': 'application/json' }},
                        }})

                        alert('Successfully verified!')
                    }})
                </script>
            </body>
        </html>", app_id = config.app_id
	};

	Ok(Html(page))
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
	Path(chat_id): Path<ChatId>,
	Path(msg_id): Path<MessageId>,
	Extension(config): Extension<AppConfig>,
	Extension(join_reqs): Extension<JoinRequests>,
	Json(req): Json<VerifyRequest>,
) -> Result<&'static str, StatusCode> {
	let join_req = join_reqs
		.get(&(chat_id, msg_id))
		.ok_or(StatusCode::NOT_FOUND)?;

	reqwest::Client::new()
		.post(format!(
			"https://developer.worldcoin.org/api/v1/verify/{}",
			config.app_id
		))
		.json(&json!({
			"signal": msg_id,
			"action": chat_id,
			"proof": req.proof,
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

	on_verified(bot, chat_id, msg_id, join_reqs)
		.await
		.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

	Ok("Verified!")
}
