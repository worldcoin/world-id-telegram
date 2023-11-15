use dashmap::DashMap;
use std::sync::Arc;
use teloxide::{
	dispatching::{MessageFilterExt, UpdateFilterExt},
	prelude::{dptree, Dispatcher},
	requests::Requester,
	types::{ChatId, Message, MessageId, Update, UserId},
	utils::command::BotCommands,
	Bot,
};

use crate::{bot::commands::Command, config::AppConfig};
pub use join_check::on_verified;

mod commands;
mod join_check;

type HandlerResult = Result<(), HandlerError>;
pub type JoinRequests = Arc<DashMap<(ChatId, UserId), JoinRequest>>;
type HandlerError = Box<dyn std::error::Error + Send + Sync>;

#[derive(Clone)]
pub struct JoinRequest {
	pub is_verified: bool,
	pub msg_id: Option<MessageId>,
}

impl JoinRequest {
	pub fn new(msg_id: MessageId) -> Self {
		Self {
			is_verified: false,
			msg_id: Some(msg_id),
		}
	}
}

pub async fn start(bot: Bot, config: AppConfig, join_requests: JoinRequests) {
	log::info!("Starting World ID bot...");

	bot.set_my_commands(Command::bot_commands())
		.await
		.expect("Failed to set commands");

	let handler = dptree::entry().branch(
		Update::filter_message()
			.branch(Message::filter_new_chat_members().endpoint(join_check::join_handler))
			.branch(Message::filter_text().endpoint(commands::command_handler)),
	);

	Dispatcher::builder(bot, handler)
		.default_handler(|_| async {})
		.dependencies(dptree::deps![Arc::new(config), join_requests])
		.enable_ctrlc_handler()
		.build()
		.dispatch()
		.await;
}
