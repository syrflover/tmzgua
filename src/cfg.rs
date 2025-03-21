use std::path::PathBuf;

use serde::Deserialize;
use serenity::{
    model::id::{ChannelId, GuildId},
    prelude::TypeMapKey,
};

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    token: String,
    guild_id: u64,
    channel_id: u64,
    cache: String,
}

impl Config {
    pub fn token(&self) -> &str {
        &self.token
    }

    pub fn guild_id(&self) -> GuildId {
        GuildId::new(self.guild_id)
    }

    pub fn channel_id(&self) -> ChannelId {
        ChannelId::new(self.channel_id)
    }

    pub fn cache(&self) -> PathBuf {
        PathBuf::from(&self.cache)
    }
}

impl TypeMapKey for Config {
    type Value = Config;
}
