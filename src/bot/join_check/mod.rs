use std::sync::Arc;

use teloxide::{
	prelude::*,
	types::{ChatPermissions, InlineKeyboardButton, InlineKeyboardMarkup, MessageId, User},
	utils::html::escape,
};

use crate::{
	bot::{commands::on_group_not_allowed, HandlerResult, JoinRequest, JoinRequests},
	config::AppConfig,
};

pub async fn join_handler(
	bot: Bot,
	msg: Message,
	users: Vec<User>,
	config: Arc<AppConfig>,
	join_requests: JoinRequests,
) -> HandlerResult {
	if !config.groups_config.is_group_allowed(msg.chat.id) {
		return on_group_not_allowed(bot, &config.groups_config, msg).await;
	}

	let chat_cfg = config.groups_config.get(msg.chat.id);

	for user in users {
		let join_requests = join_requests.clone();
		if user.is_bot {
			continue;
		}

		let welcome_msg = chat_cfg.messages.create_welcome_msg(
			&user,
			&escape(if let Some(ref title) = chat_cfg.chat_name {
				title
			} else {
				msg.chat.title().unwrap_or_default()
			}),
		);

		bot.restrict_chat_member(msg.chat.id, user.id, ChatPermissions::empty())
			.await?;

		let verify_button = InlineKeyboardButton::url(
			"Verify with World ID",
			config
				.app_url
				.join(&format!("verify/{}/{}", msg.chat.id, msg.id))?,
		);

		let msg_id = bot
			.send_message(msg.chat.id, welcome_msg)
			.reply_to_message_id(msg.id)
			.parse_mode(teloxide::types::ParseMode::Html)
			.reply_markup(InlineKeyboardMarkup::new([vec![verify_button]]))
			.await?
			.id;

		join_requests.insert((msg.chat.id, msg_id), JoinRequest::new(user.id));

		tokio::spawn({
			let bot = bot.clone();
			let ban_after = chat_cfg.ban_after;
			async move {
				tokio::time::sleep(ban_after).await;

				if let Some((_, data)) = join_requests.remove(&(msg.chat.id, msg_id)) {
					if !data.is_verified {
						bot.ban_chat_member(msg.chat.id, data.user_id)
							.await
							.expect("Failed to ban the member after timeout");

						bot.delete_message(msg.chat.id, msg_id)
							.await
							.expect("Failed to delete the message after timeout");
					}
				}
			}
		});
	}

	Ok(())
}

pub async fn on_verified(
	bot: Bot,
	chat_id: ChatId,
	msg_id: MessageId,
	join_requests: JoinRequests,
) -> HandlerResult {
	let mut join_req = join_requests
		.get_mut(&(chat_id, msg_id))
		.ok_or("Can't find the message id in group dialogue")?;

	let Some(permissions) = bot.get_chat(chat_id).await?.permissions() else {
		return Err("Can't get the group permissions".into());
	};

	join_req.is_verified = true;

	bot.restrict_chat_member(chat_id, join_req.user_id, permissions)
		.await?;

	bot.delete_message(chat_id, msg_id).await?;

	Ok(())
}
