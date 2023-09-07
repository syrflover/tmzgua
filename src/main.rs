use serenity::{
    framework::StandardFramework, futures::future::join_all, prelude::GatewayIntents, Client,
};
use songbird::SerenityInit;
use tap::Pipe;
use tmzgua::{cfg::Config, handler::Handler};

#[tokio::main]
async fn main() {
    println!("Hello, world!");

    // read config file
    let cfgs: Vec<Config> = tokio::fs::read_to_string("./cfg.json")
        .await
        .unwrap()
        .pipe(|x| serde_json::from_str(&x).unwrap());

    let intents = GatewayIntents::GUILD_VOICE_STATES
        | GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::GUILD_MESSAGE_REACTIONS
        | GatewayIntents::MESSAGE_CONTENT;

    let mut tasks = Vec::new();

    for cfg in cfgs {
        // create cache directory
        tokio::fs::create_dir_all(cfg.cache()).await.unwrap();

        let task = tokio::spawn(async move {
            let framework = StandardFramework::new();

            let mut client = Client::builder(cfg.token(), intents)
                .event_handler(Handler)
                .framework(framework)
                .register_songbird()
                .await
                .unwrap();

            {
                let mut x = client.data.write().await;
                x.insert::<Config>(cfg);
            }

            client.start().await.unwrap();
        });

        tasks.push(task);
    }

    for res in join_all(tasks).await {
        if let Err(err) = res {
            eprintln!("{err}");
        }
    }
}
