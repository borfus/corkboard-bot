use std::env;

use serenity::async_trait;
use serenity::prelude::*;
use serenity::framework::standard::StandardFramework;
use serenity::framework::standard::macros::group;

mod commands;
use commands::pins::*;

#[group]
#[commands(pins)]
pub struct PinsGeneral;

struct Handler;

#[async_trait]
impl EventHandler for Handler {}

#[tokio::main]
async fn main() {
    let framework = StandardFramework::new()
        .configure(|c| c.prefix("_")) // set the bot's prefix to "_"
        .group(&PINSGENERAL_GROUP);

    // Login with a bot token from the environment
    let token = env::var("DISCORD_TOKEN").expect("token");
    let intents = GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT;
    let mut client = Client::builder(token, intents)
        .event_handler(Handler)
        .framework(framework)
        .await
        .expect("Error creating client");

    // start listening for events by starting a single shard
    if let Err(why) = client.start().await {
        println!("An error occurred while running the client: {:?}", why);
    }
}

