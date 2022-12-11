use std::env;
use std::collections::HashSet;

use serenity::async_trait;
use serenity::prelude::*;
use serenity::framework::standard::{StandardFramework, help_commands, HelpOptions, CommandGroup, CommandResult, Args};
use serenity::framework::standard::macros::{group, help};
use serenity::model::channel::Message;
use serenity::model::id::UserId;

mod commands;
use commands::{pins::*, events::*, faqs::*, list::*, luckymon::*};

#[group]
#[commands(pins, events, faqs, list, luckymon)]
pub struct General;

struct Handler;

#[async_trait]
impl EventHandler for Handler {}

#[tokio::main]
async fn main() {
    let framework = StandardFramework::new()
        .configure(|c| c.prefix(".")) // set the bot's prefix to "."
        .group(&GENERAL_GROUP)
        .help(&HELP);

    // Login with a bot token from the environment
    let token = env::var("DISCORD_TOKEN").expect("Unable to retrieve DISCORD_TOKEN environment variable!");
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

#[help]
#[command_not_found_text = "Could not find: `{}`."]
#[max_levenshtein_distance(3)]
#[indention_prefix = "+"]
#[lacking_permissions = "Hide"]
#[lacking_role = "Hide"]
#[wrong_channel = "Hide"]
async fn help(
    context: &Context,
    msg: &Message,
    args: Args,
    help_options: &'static HelpOptions,
    groups: &[&'static CommandGroup],
    owners: HashSet<UserId>,
) -> CommandResult {
    let _ = help_commands::with_embeds(context, msg, args, help_options, groups, owners).await;
    Ok(())
}

