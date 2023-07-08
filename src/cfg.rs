use std::path::Path;

use serde::Deserialize;
use serenity::{
    model::id::{ChannelId, GuildId},
    prelude::TypeMapKey,
};

#[derive(Debug, Deserialize)]
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
        GuildId(self.guild_id)
    }

    pub fn channel_id(&self) -> ChannelId {
        ChannelId(self.channel_id)
    }

    pub fn cache(&self) -> &Path {
        Path::new(&self.cache)
    }
}

impl TypeMapKey for Config {
    type Value = Config;
}
