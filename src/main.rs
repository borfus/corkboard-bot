use std::env;

use serenity::async_trait;
use serenity::model::application::interaction::Interaction;
use serenity::model::gateway::Ready;
use serenity::model::guild::Guild;
use serenity::model::id::GuildId;
use serenity::prelude::*;

mod commands;
mod pacific;
mod slash;
mod validation;

use commands::luckymon;

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _ctx: Context, ready: Ready) {
        println!("{} is connected.", ready.user.name);
    }

    /// Commands are declared once the cache has every guild, rather than in
    /// `ready`: at ready time guilds are still marked unavailable and their ids
    /// are not reliable to register against.
    async fn cache_ready(&self, ctx: Context, guilds: Vec<GuildId>) {
        for guild_id in guilds {
            match slash::registry::register(&ctx, guild_id).await {
                Ok(()) => println!("Registered slash commands in guild {}.", guild_id),
                Err(why) => {
                    println!("Failed to register commands in guild {}: {:?}", guild_id, why)
                }
            }
        }
    }

    /// A guild joined after startup needs the same treatment, or the bot sits
    /// there with no commands at all.
    async fn guild_create(&self, ctx: Context, guild: Guild, is_new: bool) {
        if !is_new {
            return;
        }
        match slash::registry::register(&ctx, guild.id).await {
            Ok(()) => println!("Registered slash commands in new guild {}.", guild.id),
            Err(why) => println!("Failed to register in new guild {}: {:?}", guild.id, why),
        }
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::ApplicationCommand(command) = interaction {
            slash::registry::dispatch(&ctx, &command).await;
        }
    }
}

#[tokio::main]
async fn main() {
    let token =
        env::var("DISCORD_TOKEN").expect("Unable to retrieve DISCORD_TOKEN environment variable!");

    // No MESSAGE_CONTENT any more: nothing reads message text now that every
    // command arrives as an interaction. That privileged intent was only ever
    // needed for the "." prefix.
    let intents = GatewayIntents::non_privileged();

    let mut client = Client::builder(token, intents)
        .event_handler(Handler)
        .await
        .expect("Error creating client");

    let _ = luckymon::initialize().await;

    if let Err(why) = client.start().await {
        println!("An error occurred while running the client: {:?}", why);
    }
}
