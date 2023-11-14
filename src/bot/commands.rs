use std::sync::Arc;
use teloxide::{
	payloads::SendMessageSetters,
	requests::Requester,
	types::{Me, Message},
	utils::command::BotCommands,
	Bot,
};

use crate::{bot::HandlerResult, config::GroupsConfig};

#[derive(BotCommands)]
#[command(rename_rule = "lowercase", description = "Available commands:")]
pub enum Command {
	#[command(description = "Display this text")]
	Help,
	#[command(description = "Pong!")]
	Ping,
	#[command(description = "Start the bot")]
	Start,
}

pub async fn command_handler(
	bot: Bot,
	config: Arc<GroupsConfig>,
	msg: Message,
	me: Me,
	text: String,
) -> HandlerResult {
	if msg.from().is_none() {
		return Ok(());
	}

	if !config.is_group_allowed(msg.chat.id) {
		return on_group_not_allowed(bot, config, msg).await;
	}

	let Ok(command) = BotCommands::parse(text.as_str(), me.username()) else {
		return Ok(());
	};

	match command {
		Command::Help => {
			bot.send_message(msg.chat.id, Command::descriptions().to_string())
				.reply_to_message_id(msg.id)
				.await?;
		},
		Command::Ping => {
			if msg.chat.is_private() {
				bot.send_message(msg.chat.id, "pong")
					.reply_to_message_id(msg.id)
					.await?;
				return Ok(());
			}

			let is_admin = bot
				.get_chat_member(msg.chat.id, bot.get_me().await?.id)
				.await?
				.is_administrator();

			if is_admin {
				bot.send_message(msg.chat.id, "Bot has admin permissions and is ready to go!")
					.reply_to_message_id(msg.id)
					.await?;
			} else {
				bot.send_message(msg.chat.id, "Bot doesn't have admin permissions! Please, give it admin permissions and try again.").reply_to_message_id(msg.id)
					.await?;
			}
		},
		Command::Start => {
			if msg.chat.is_private() {
				bot.send_message(msg.chat.id, "Hello! To use this bot, add it to a group and give it admin permissions. Then, use /ping to check if it's working.").reply_to_message_id(msg.id).await?;
			}
		},
	};

	Ok(())
}

pub async fn on_group_not_allowed(
	bot: Bot,
	config: Arc<GroupsConfig>,
	msg: Message,
) -> HandlerResult {
	log::error!(
		"Unknown chat {} with id {}",
		msg.chat.title().unwrap_or_default(),
		msg.chat.id
	);

	bot.send_message(
		msg.chat.id,
		&config.get(msg.chat.id).messages.unauthorized_group,
	)
	.await?;
	bot.leave_chat(msg.chat.id).await?;

	Ok(())
}
