use std::sync::Arc;
use teloxide::{
	payloads::SendMessageSetters,
	requests::Requester,
	types::{Me, Message, ReplyParameters},
	utils::command::BotCommands,
	Bot,
};

use crate::{
	bot::HandlerResult,
	config::{AppConfig, GroupsConfig},
};

#[derive(BotCommands)]
#[command(rename_rule = "lowercase", description = "Available commands:")]
pub enum Command {
	#[command(description = "Explain how this bot works, and how to use it.")]
	Help,
	#[command(description = "Check that the bot is online and has the right permissions.")]
	Check,
	#[command(description = "Initial help when talking to the bot for the first time.")]
	Start,
}

pub async fn command_handler(
	bot: Bot,
	config: Arc<AppConfig>,
	msg: Message,
	me: Me,
	text: String,
) -> HandlerResult {
	if msg.from.is_none() {
		return Ok(());
	}

	if !config.groups_config.is_group_allowed(msg.chat.id) {
		return on_group_not_allowed(bot, &config.groups_config, msg).await;
	}

	let Ok(command) = BotCommands::parse(text.as_str(), me.username()) else {
		return Ok(());
	};

	match command {
		Command::Check => {
			if msg.chat.is_private() {
				bot.send_message(msg.chat.id, "You can only use this bot in public groups. Please add me to a public group (with admin permissions) and try again.")
					.reply_parameters(ReplyParameters::new(msg.id))
					.await?;
				return Ok(());
			}

			let is_admin = bot
				.get_chat_member(msg.chat.id, bot.get_me().await?.id)
				.await?
				.is_administrator();

			if is_admin {
				bot.send_message(msg.chat.id, "Bot has admin permissions and is ready to go! Once someone joins the group, they'll be asked to prove they're human with World ID before they can send messages.")
					.reply_parameters(ReplyParameters::new(msg.id))
					.await?;
			} else {
				bot.send_message(msg.chat.id, "Bot doesn't have admin permissions! Please, give it admin permissions and try again.")
					.reply_parameters(ReplyParameters::new(msg.id))
					.await?;
			}
		},
		Command::Help | Command::Start => {
			if msg.chat.is_private() {
				bot.send_message(msg.chat.id, r#"
Welcome to the World ID Telegram bot!

You can use me to protect your group from spammers and bots. To get started, add me to your (public) group and give me admin permissions. When someone joins your group, they'll be asked to prove they're human with World ID before they can send messages.
                "#)
					.reply_parameters(ReplyParameters::new(msg.id))
					.await?;
			}
		},
	};

	Ok(())
}

pub async fn on_group_not_allowed(bot: Bot, config: &GroupsConfig, msg: Message) -> HandlerResult {
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
