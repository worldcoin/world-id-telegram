use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;
use serde_with::{serde_as, DisplayFromStr};
use std::{collections::HashMap, time::Duration};
use teloxide::{
	types::{ChatId, User, UserId},
	utils::html::escape,
};

#[derive(Debug, Clone, Default, Deserialize)]
pub struct AppConfig {
	pub app_id: String,
	pub bot_token: String,
	#[serde(flatten, default)]
	pub groups_config: GroupsConfig,
}

impl AppConfig {
	pub fn try_read() -> Result<AppConfig, ConfigError> {
		Config::builder()
			.add_source(File::with_name("config.toml").required(false))
			.add_source(File::with_name("config.dev.toml").required(false))
			.add_source(Environment::with_prefix("WLD_CAPTCHA"))
			.build()?
			.try_deserialize()
	}
}

#[serde_as]
#[derive(Debug, Clone, Default, Deserialize)]
pub struct GroupsConfig {
	/// List of allowed groups, if `None` bot will allow all groups
	#[serde(default)]
	pub allowed_group_ids: Vec<ChatId>,
	#[serde(default)]
	fallback_group_settings: GroupSettings,
	#[serde_as(as = "HashMap<DisplayFromStr, _>")]
	#[serde(default)]
	group_settings: HashMap<i64, GroupSettings>,
}

impl GroupsConfig {
	pub fn is_group_allowed(&self, chat_id: ChatId) -> bool {
		self.allowed_group_ids.is_empty() || self.allowed_group_ids.contains(&chat_id)
	}

	pub fn get(&self, chat_id: ChatId) -> &GroupSettings {
		match self.group_settings.get(&chat_id.0) {
			Some(s) => s,
			None => &self.fallback_group_settings,
		}
	}
}

#[derive(Debug, Clone, Deserialize)]
pub struct GroupSettings {
	pub chat_name: Option<String>,
	/// List of allowed admin to use bot commands and admin related stuff
	pub admin_ids: Option<Vec<UserId>>,
	#[serde(with = "humantime_serde")]
	pub ban_after: Duration,
	#[serde(default)]
	pub messages: MessagesText,
}

impl Default for GroupSettings {
	fn default() -> Self {
		Self {
			chat_name: None,
			admin_ids: None,
			messages: MessagesText::default(),
			ban_after: Duration::from_secs(60 * 5),
		}
	}
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct MessagesText {
	pub new_user_template: String,
	pub unauthorized_group: String,
	pub successfully_verified: String,
	pub user_doesnt_match_error: String,
}

impl MessagesText {
	pub fn create_welcome_msg(&self, user: &User, chat_name: &str) -> String {
		self.new_user_template
			.replace("{TAGUSER}", &user.mention().unwrap())
			.replace("{CHATNAME}", &escape(chat_name))
	}
}

impl Default for MessagesText {
	fn default() -> Self {
		Self {
            user_doesnt_match_error: "❌ This message isn't for you".to_string(),
			unauthorized_group: "❌ You can't use this bot on this group. Bye!".to_string(),
			successfully_verified: "✅ Verified with World ID. Welcome to the group!".to_string(),
            new_user_template: "👋 gm {TAGUSER}! Welcome to {CHATNAME}.\nTo access the group, please verify your account with World ID.".to_string(),
		}
	}
}
