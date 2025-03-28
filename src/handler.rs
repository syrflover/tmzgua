use std::{
    hash::{Hash, Hasher},
    io,
    path::Path,
    sync::Arc,
    time::Duration,
};

use fnv::FnvHasher;
use regex::Regex;
use serenity::{
    all::ActivityData,
    model::{
        channel::Message,
        gateway::Ready,
        id::{ChannelId, GuildId, UserId},
        prelude::{MessageType, ReactionType},
    },
    prelude::*,
};
use songbird::{
    error::{ControlError, JoinError},
    input::Input,
    tracks::PlayMode,
    Call,
};
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncWriteExt},
    process::Command,
    time::sleep,
};
use uuid::{Timestamp, Uuid};

use crate::{cfg::Config, say_cache::SayCache};

fn filter_regex(x: &str) -> bool {
    // const replaceRegExp: [RegExp, string][] = [
    //     // [/-|"|\\|'|\||`|\$/g, ''], // bug fix

    //     [/~/g, ''], // bug fix
    //     [/<@[0-9]+>/g, ''], // user id
    //     [/<#[0-9]+>/g, ''], // channel id
    //     [/<:.+:[0-9]+>/g, ''], // custom emoji id
    // ];

    // const ignoreRegExp: RegExp[] = [
    //     /(http|https|ftp|telnet|news|mms):\/\/[^\"'\s()]+/i, // url
    //     /```.+```/is, // code block
    // ];
    let url_regex = Regex::new(r#"(?i)(http|https|ftp|telnet|news|mms)://[^"'\s()]+"#).unwrap();
    let code_block_regex = Regex::new(r"(?s)```.+```").unwrap();
    let user_id_regex = Regex::new(r"<@[0-9]+>").unwrap();
    let channel_id_regex = Regex::new(r#"<#[0-9]+>"#).unwrap();
    let custom_emoji_id_regex = Regex::new(r#"<:\w+:[0-9]+>"#).unwrap();
    let external_custom_emoji_regex = Regex::new(r#"<\w{0,1}:[0-9]+>"#).unwrap(); // <a:DDo:1055872203473825852>

    url_regex.find(x).is_some()
        || code_block_regex.find(x).is_some()
        || user_id_regex.find(x).is_some()
        || channel_id_regex.find(x).is_some()
        || custom_emoji_id_regex.find(x).is_some()
        || external_custom_emoji_regex.find(x).is_some()
}

async fn get_voice_handler(
    ctx: &Context,
    guild_id: GuildId,
    channel_id: ChannelId,
) -> Result<Arc<Mutex<Call>>, JoinError> {
    let manager = songbird::get(ctx).await.unwrap().clone();

    manager.join(guild_id, channel_id).await
}

#[cfg(target_os = "macos")]
async fn make_siri_voice(p: impl AsRef<Path>, content: &str) -> anyhow::Result<Input> {
    use anyhow::anyhow;

    let p = p.as_ref().to_path_buf();

    if !p.exists() {
        let say = Command::new("say")
            .arg(content)
            .arg("-o")
            .arg(p.as_os_str())
            // .args(["--file-format", "aiff", "--data-format", "aac"])
            .output();

        tokio::select! {
            _ = sleep(Duration::from_secs(6)) => {
                return Err(anyhow!("timeout"));
            }
            res = say => {
              res?;
            }
        }
    }

    let r = songbird::input::File::new(p);

    // r.raw.spawn_loader();

    Ok(r.into())

    // Ok(encode_to_source(File::open(p).await?.into_std().await)
    //     .await
    //     .unwrap())

    // match File::open(p).await {
    //     Ok(r) => Ok(encode_to_source(r.into_std().await).await.unwrap()),
    //     Err(err) if err.kind() == io::ErrorKind::NotFound => {
    //         // TODO: err 구분해야됨
    //         Err(err)
    //     }
    //     Err(err) => Err(err),
    // }
}

pub struct Handler;

#[async_trait::async_trait]
impl EventHandler for Handler {
    // async fn voice_state_update(
    //     &self,
    //     ctx: Context,
    //     _old_state: Option<VoiceState>,
    //     new_state: VoiceState,
    // ) {
    //     println!("{new_state:?}");

    //     let VoiceState {
    //         guild_id,
    //         channel_id,
    //         user_id,
    //         ..
    //     } = new_state;

    //     let Some(guild_id) = guild_id else { return };

    //     let Some(channel_id) = channel_id else { return };

    //     let Ok(user) = user_id.to_user(&ctx.http).await else {
    //         return;
    //     };

    //     if user.bot {
    //         return;
    //     }

    //     let mut x = ctx.data.write().await;
    //     let say_cache = x.get_mut::<SayCache>().unwrap();

    //     say_cache.users.remove(&user.id);

    //     if say_cache.users.is_empty() {
    //         let Ok(handler) = get_voice_handler(&ctx, guild_id, channel_id).await else {
    //             return;
    //         };

    //         handler.lock().await.leave().await.ok();
    //     }
    // }

    async fn message(&self, ctx: Context, message: Message) {
        let cfg = {
            let x = ctx.data.read().await;
            x.get::<Config>().unwrap().clone()
        };

        // println!("{};length={}", message.content, message.content.len());

        if message.guild_id.filter(|x| *x == cfg.guild_id()).is_none() {
            return;
        }

        if message.kind != MessageType::Regular {
            return;
        }

        if message.content == "> help" {
            let help_message = r"
마지막으로 활성화한 시간 또는 말한 시간 기준으로 4시간동안 아무 말도 하지 않으면 자동으로 비활성 돼요.
`> sayEnable`
`> sayEnable @GORANI`
`> sayDisable`";

            if let Err(err) = message.reply(&ctx.http, help_message).await {
                eprintln!("{err}");

                message
                    .react(&ctx.http, ReactionType::Unicode("❌".to_owned()))
                    .await
                    .ok();
            }

            return;
        }

        fn parse_user_id(x: &str) -> Option<UserId> {
            if x.starts_with("<@") && x.ends_with('>') {
                x[2..x.len() - 1].parse().ok().map(|x| UserId::new(x))
            } else {
                None
            }
        }

        if message.content.starts_with("> sayEnable") {
            let target_user = parse_user_id(message.content.replace("> sayEnable", "").trim())
                .unwrap_or(message.author.id);

            {
                let mut x = ctx.data.write().await;
                let say_cache = x.get_mut::<SayCache>().unwrap();

                say_cache
                    .users
                    .insert(target_user, (), Duration::from_secs(3600 * 4));

                if let Err(err) = save_enabled_users(say_cache.to_vec(), &say_cache.path).await {
                    eprintln!("{err}");

                    message
                        .react(&ctx.http, ReactionType::Unicode("❌".to_owned()))
                        .await
                        .ok();
                }
            }

            message
                .react(&ctx.http, ReactionType::Unicode("✅".to_owned()))
                .await
                .ok();

            return;
        }

        if message.content == "> sayDisable" {
            // 악용 가능성 높음. 가령 다른 사람이 임의로 비활성화 한다던가
            // let target_user = parse_user_id(message.content.replace("> sayDisable", "").trim())
            //     .unwrap_or(message.author.id);

            {
                let mut x = ctx.data.write().await;
                let say_cache = x.get_mut::<SayCache>().unwrap();

                let _r = say_cache.users.remove(&message.author.id);

                if let Err(err) = save_enabled_users(say_cache.to_vec(), &say_cache.path).await {
                    eprintln!("{err}");

                    message
                        .react(&ctx.http, ReactionType::Unicode("❌".to_owned()))
                        .await
                        .ok();
                }
            }

            message
                .react(&ctx.http, ReactionType::Unicode("✅".to_owned()))
                .await
                .ok();

            return;
        }

        let (say, cache_path) = {
            let mut x = ctx.data.write().await;
            let say_cache = x.get_mut::<SayCache>().unwrap();

            let r = say_cache.users.get(&message.author.id).is_some();

            if r {
                say_cache
                    .users
                    .insert(message.author.id, (), Duration::from_secs(3600 * 4));
            }

            (r, say_cache.path.clone())
        };

        if !say {
            return;
        }

        if filter_regex(&message.content) {
            return;
        }

        let uuid = Uuid::new_v7(Timestamp::now(uuid::ContextV7::new()));

        #[cfg(target_os = "macos")]
        let save_path = cache_path.join(format!("{uuid}.aiff"));
        #[cfg(target_os = "windows")]
        let save_path = todo!();

        let (guild_id, channel_id) = {
            let x = ctx.data.read().await;
            let x = x.get::<Config>().unwrap();
            (x.guild_id(), x.channel_id())
        };

        let handler = match get_voice_handler(&ctx, guild_id, channel_id).await {
            Ok(r) => r,
            Err(err) => {
                eprintln!("{err}");

                message
                    .react(&ctx.http, ReactionType::Unicode("❌".to_owned()))
                    .await
                    .ok();

                match err {
                    JoinError::Dropped | JoinError::Driver(_) => {
                        // TODO: restart bot
                    }
                    _ => {}
                }

                return;
            }
        };
        let mut handler = handler.lock().await;

        handler.stop();

        #[cfg(target_os = "macos")]
        let mut source = match make_siri_voice(&save_path, &message.content).await {
            Ok(r) => r,
            Err(err) => {
                eprintln!("{err}");

                message
                    .react(&ctx.http, ReactionType::Unicode("❌".to_owned()))
                    .await
                    .ok();

                return;
            }
        };
        #[cfg(target_os = "windows")]
        let mut source = todo!();

        let mut track = handler.play_only_input(source);

        let mut try_count = 0;

        loop {
            if try_count > 3 {
                // TODO: err
                return;
            }

            // println!("try_count = {try_count}");

            try_count += 1;

            let play_result = [track.set_volume(0.225), track.play()]
                .into_iter()
                .collect::<Result<(), _>>();

            let play_state = if let Err(ControlError::Finished) = play_result {
                PlayMode::End
            } else {
                // sleep(Duration::from_millis(100)).await;

                track
                    .get_info()
                    .await
                    .map(|x| x.playing)
                    .unwrap_or(PlayMode::End)
            };

            match play_state {
                PlayMode::Play => break,

                PlayMode::End => {
                    #[cfg(target_os = "macos")]
                    {
                        match make_siri_voice(&save_path, &message.content).await {
                            Ok(r) => {
                                source = r;
                            }
                            Err(err) => {
                                eprintln!("{err}");

                                message
                                    .react(&ctx.http, ReactionType::Unicode("❌".to_owned()))
                                    .await
                                    .ok();
                                return;
                            }
                        }
                    }
                    #[cfg(target_os = "windows")]
                    {
                        source = todo!();
                    }

                    track = handler.play_only_input(source);
                }

                _ => {}
            }
        }

        let enabled_users = {
            let x = ctx.data.read().await;
            let say_cache = x.get::<SayCache>().unwrap();

            say_cache.to_vec()
        };

        if let Err(err) = save_enabled_users(enabled_users, &cache_path).await {
            eprintln!("{err}");

            message
                .react(&ctx.http, ReactionType::Unicode("❌".to_owned()))
                .await
                .ok();
        }

        if let Err(err) = tokio::fs::remove_file(save_path).await {
            tracing::error!("{err}");
        };
    }

    async fn ready(&self, ctx: Context, _ready: Ready) {
        ctx.set_activity(Some(ActivityData::playing("> help")));

        let mut x = ctx.data.write().await;

        let cache_path = x.get::<Config>().unwrap().cache().to_owned();

        let mut xs = Vec::new();

        let users: Vec<UserId> = match File::open(cache_path.join("users.json")).await {
            Ok(mut r) => {
                r.read_to_end(&mut xs).await.unwrap();
                serde_json::from_slice(&xs).unwrap()
            }
            Err(err) if err.kind() == io::ErrorKind::NotFound => Vec::new(),
            Err(err) => {
                panic!("{err}");
            }
        };

        x.insert::<SayCache>(SayCache::from((users, cache_path.as_path())));

        println!("{} is ready", ctx.cache.current_user().name);
    }
}

async fn save_enabled_users(enabled_users: Vec<UserId>, cache_path: &Path) -> io::Result<()> {
    let p = cache_path.join("users.json");

    let mut f = match File::create(&p).await {
        Ok(r) => r,
        Err(err) if err.kind() == io::ErrorKind::AlreadyExists => File::open(&p).await?,
        Err(err) => {
            return Err(err);
        }
    };

    f.write_all(&serde_json::to_vec(&enabled_users).unwrap())
        .await?;

    Ok(())
}
