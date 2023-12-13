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
	types::{ChatId, User, UserId},
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
		.route("/health", get(|| async { "OK" }))
		.route(
			"/verify/:chat_id/:user_id",
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
	Extension(config): Extension<AppConfig>,
	Path((chat_id, user_id)): Path<(ChatId, UserId)>,
	Extension(join_reqs): Extension<JoinRequests>,
) -> Result<Html<String>, StatusCode> {
	let join_req = join_reqs
		.get(&(chat_id, user_id))
		.ok_or(StatusCode::NOT_FOUND)?;
	let msg_id = join_req.msg_id.ok_or(StatusCode::CONFLICT)?;

	let page = formatdoc! {"<!DOCTYPE html>
        <html lang=\"en\">
            <head>
                <meta charset=\"UTF-8\" />
                <meta http-equiv=\"X-UA-Compatible\" content=\"IE=edge\" />
                <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\" />
                <title>Verify with World ID</title>
            </head>
            <body>
                <script src=\"https://unpkg.com/@worldcoin/idkit-standalone/build/index.global.js\"></script>

                <script>
                    IDKit.init({{
                        autoClose: true,
                        signal: '{msg_id}',
                        app_id: '{app_id}',
                        action: '{chat_id}',
                        enableTelemetry: true,
                        credential_types: ['phone', 'orb'],
                    }})

                    window.addEventListener('load', async () => {{
                        const res = await fetch('/verify/{chat_id}/{user_id}', {{
                            method: 'POST',
                            body: JSON.stringify(await IDKit.open()),
                            headers: {{ 'Content-Type': 'application/json' }},
                        }})

                        if (res.ok) alert('Successfully verified! You can now close this and go back to the group.')
                        else if (res.status === 429) alert('This World ID has already been used to join this group. You can\\'t do it again!')
                        else alert('Something went wrong, please try again later.')

                        window.close()
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
	Extension(config): Extension<AppConfig>,
	Path((chat_id, user_id)): Path<(ChatId, UserId)>,
	Extension(join_reqs): Extension<JoinRequests>,
	Json(req): Json<VerifyRequest>,
) -> Result<&'static str, StatusCode> {
	let join_req = join_reqs
		.get(&(chat_id, user_id))
		.ok_or(StatusCode::NOT_FOUND)?;
	let msg_id = join_req.msg_id.ok_or(StatusCode::CONFLICT)?;

	let req = reqwest::Client::new()
		.post(format!(
			"https://developer.worldcoin.org/api/v1/verify/{}",
			config.app_id
		))
		.header("User-Agent", "World ID Telegram Bot/1.0")
		.json(&json!({
			"proof": req.proof,
			"signal": msg_id.to_string(),
			"action": chat_id.to_string(),
			"merkle_root": req.merkle_root,
			"nullifier_hash": req.nullifier_hash,
			"credential_type": req.credential_type,
		}))
		.send()
		.await
		.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

	if req.status().is_client_error() || req.status().is_server_error() {
		let res = req.json::<serde_json::Value>().await.map_err(|e| {
			log::error!("Failed to deserialize dev portal body: {e:?}");

			StatusCode::INTERNAL_SERVER_ERROR
		})?;

		let Some(code) = res.get("code") else {
			log::error!("Developer Portal returned error: {:?}", res);

			return Err(StatusCode::BAD_REQUEST);
		};

		if code.as_str() == Some("max_verifications_reached") {
			return Err(StatusCode::TOO_MANY_REQUESTS);
		}

		log::error!("Failed to verify proof: {:?}", res);
		return Err(StatusCode::BAD_REQUEST);
	}

	drop(join_req);

	on_verified(bot, chat_id, user_id, join_reqs)
		.await
		.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

	Ok("Verified!")
}
