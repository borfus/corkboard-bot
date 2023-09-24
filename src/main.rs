use std::collections::HashSet;
use std::env;

use serenity::async_trait;
use serenity::framework::standard::macros::{group, help};
use serenity::framework::standard::{
    help_commands, Args, CommandGroup, CommandResult, HelpOptions, StandardFramework,
};
use serenity::model::channel::Message;
use serenity::model::id::UserId;
use serenity::prelude::*;

mod commands;
use commands::{
    events::*,
    faqs::*,
    luckydex::*,
    luckymon::{self, *},
    pins::*,
};

mod validation;

#[group]
#[commands(pins, events, faqs, luckymon, luckydex)]
pub struct General;

#[group]
#[commands(
    add_faq,
    edit_faq,
    delete_faq,
    add_event,
    edit_event,
    delete_event,
    add_pin,
    edit_pin,
    delete_pin
)]
pub struct Admin;

struct Handler;

#[async_trait]
impl EventHandler for Handler {}

#[tokio::main]
async fn main() {
    let framework = StandardFramework::new()
        .configure(|c| c.prefix(".")) // set the bot's prefix to "."
        .group(&GENERAL_GROUP)
        .group(&ADMIN_GROUP)
        .help(&HELP);

    // Login with a bot token from the environment
    let token =
        env::var("DISCORD_TOKEN").expect("Unable to retrieve DISCORD_TOKEN environment variable!");
    let intents = GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT;
    let mut client = Client::builder(token, intents)
        .event_handler(Handler)
        .framework(framework)
        .await
        .expect("Error creating client");

    let _ = luckymon::initialize().await;

    // start listening for events by starting a single shard
    if let Err(why) = client.start().await {
        println!("An error occurred while running the client: {:?}", why);
    }
}

#[help]
#[command_not_found_text = "Could not find: `{}`."]
#[max_levenshtein_distance(3)]
#[indention_prefix = "+"]
#[lacking_permissions = "Nothing"]
#[wrong_channel = "Nothing"]
#[lacking_conditions = "Nothing"]
#[lacking_role = "Nothing"]
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
