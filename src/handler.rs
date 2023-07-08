use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    io,
    path::Path,
    sync::Arc,
    time::Duration,
};

use serenity::{
    model::{
        channel::Message,
        gateway::Ready,
        id::{ChannelId, GuildId},
        prelude::ReactionType,
    },
    prelude::*,
};
use songbird::{
    error::JoinError,
    input::Input,
    tracks::{PlayMode, TrackError},
    Call,
};
use tokio::{fs::File, process::Command};

use crate::{cfg::Config, encode_to_source::encode_to_source, say_cache::SayCache};

async fn get_voice_handler(
    ctx: &Context,
    guild_id: GuildId,
    channel_id: ChannelId,
) -> Result<Arc<Mutex<Call>>, JoinError> {
    let manager = songbird::get(ctx).await.unwrap().clone();

    let (handler_lock, join_result) = manager.join(guild_id, channel_id).await;

    join_result.map(|_| handler_lock)
}

async fn make_siri_voice(p: impl AsRef<Path>, content: &str) -> io::Result<Input> {
    let p = p.as_ref();

    if !p.exists() {
        Command::new("say")
            .arg(content)
            .arg("-o")
            .arg(p.as_os_str())
            .output()
            .await?;
    }

    match File::open(p).await {
        Ok(r) => Ok(encode_to_source(r.into_std().await).await.unwrap()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            // return Ok(None)
            // TODO: reaction 'X'
            // TODO: err 구분해야됨
            Err(err)
        }
        Err(err) => {
            // return Err(err.into())
            // TODO: reaction 'X'
            Err(err)
        }
    }
}

pub struct Handler;

#[async_trait::async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, message: Message) {
        // println!("{};length={}", message.content, message.content.len());

        if message.content == "> sayEnable" {
            {
                let mut x = ctx.data.write().await;
                let say_cache = x.get_mut::<SayCache>().unwrap();

                say_cache
                    .users
                    .insert(message.author.id, (), Duration::from_secs(300));
            }

            if let Err(err) = message
                .react(&ctx.http, ReactionType::Unicode("✅".to_owned()))
                .await
            {
                eprintln!("{err}");
            }

            return;
        }

        if message.content == "> sayDisable" {
            {
                let mut x = ctx.data.write().await;
                let say_cache = x.get_mut::<SayCache>().unwrap();

                let _r = say_cache.users.remove(&message.author.id);
            }

            if let Err(err) = message
                .react(&ctx.http, ReactionType::Unicode("✅".to_owned()))
                .await
            {
                eprintln!("{err}");
            }

            return;
        }

        let (say, cache_path) = {
            let mut x = ctx.data.write().await;
            let say_cache = x.get_mut::<SayCache>().unwrap();

            let r = say_cache.users.get(&message.author.id).is_some();

            say_cache
                .users
                .insert(message.author.id, (), Duration::from_secs(300));

            (r, say_cache.path.clone())
        };

        if !say {
            return;
        }
        // TODO: say

        let mut hasher = DefaultHasher::new();
        message.content.hash(&mut hasher);
        let hashed = hasher.finish();

        let save_path = cache_path.join(format!("{hashed}.aiff"));

        let (guild_id, channel_id) = {
            let x = ctx.data.read().await;
            let x = x.get::<Config>().unwrap();
            (x.guild_id(), x.channel_id())
        };

        let handler = get_voice_handler(&ctx, guild_id, channel_id).await.unwrap();
        let mut handler = handler.lock().await;

        handler.stop();

        let mut source = make_siri_voice(&save_path, &message.content).await.unwrap();
        let mut track = handler.play_only_source(source);

        let mut try_count = 0;

        loop {
            let play_state;

            if try_count > 3 {
                // TODO: err
                return;
            }

            // println!("try_count = {try_count}");

            try_count += 1;

            let play_result = [track.set_volume(0.15), track.play()]
                .into_iter()
                .collect::<Result<(), _>>();

            if let Err(TrackError::Finished) = play_result {
                play_state = PlayMode::End;
            } else {
                // sleep(Duration::from_millis(100)).await;

                play_state = track
                    .get_info()
                    .await
                    .map(|x| x.playing)
                    .unwrap_or(PlayMode::End);
            }

            match play_state {
                PlayMode::Play => break,

                PlayMode::End => {
                    source = make_siri_voice(&save_path, &message.content).await.unwrap();
                    track = handler.play_only_source(source);
                }

                _ => {}
            }
        }
    }

    async fn ready(&self, ctx: Context, _ready: Ready) {
        let mut x = ctx.data.write().await;

        let cache_path = x.get::<Config>().unwrap().cache().to_owned();

        // TODO: synchronize sayEnable user list to json
        x.insert::<SayCache>(SayCache::new(&cache_path));

        println!("ready");
    }
}
