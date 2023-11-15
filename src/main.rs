use dashmap::DashMap;
use dotenvy::dotenv;
use std::sync::Arc;
use teloxide::{
	requests::Requester,
	types::{ChatId, UserId},
	Bot,
};

use crate::{
	bot::{JoinRequest, JoinRequests},
	config::AppConfig,
};

mod bot;
mod config;
mod server;

#[tokio::main]
async fn main() {
	dotenv().ok();
	pretty_env_logger::init();

	let config = AppConfig::try_read().expect("Failed to read config");
	let join_requests: JoinRequests = Arc::new(DashMap::<(ChatId, UserId), JoinRequest>::new());

	let bot = Bot::new(&config.bot_token);
	let bot_data = bot.get_me().await.expect("Failed to get bot account");

	tokio::join!(
		bot::start(bot.clone(), config.clone(), join_requests.clone()),
		server::start(bot, config, bot_data.user, join_requests)
	);
}
