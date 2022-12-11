use std::collections::HashMap;

use serenity::prelude::*;
use serenity::model::channel::Message;
use serenity::framework::standard::macros::command;
use serenity::framework::standard::CommandResult;
use serenity::model::Timestamp;
use serenity::model::id::UserId;
use rustemon::pokemon::pokemon;
use rustemon::client::RustemonClient;
use rustemon::model::pokemon::Pokemon;
use chrono::NaiveDate;

pub struct LuckymonDailyEntry;

impl TypeMapKey for LuckymonDailyEntry {
    type Value = HashMap<(UserId, NaiveDate), i64>;
}

async fn write_daily_entry(ctx: &Context, user_id: UserId, lucky_num: i64) {
    let today = Timestamp::now().date_naive();
    let mut data = ctx.data.write().await;
    let daily_entry = data.get_mut::<LuckymonDailyEntry>().unwrap();
    let entry = daily_entry.entry((user_id, today)).or_insert(lucky_num);
    *entry = lucky_num;
}

async fn read_daily_entry(ctx: &Context, user_id: UserId) -> Option<i64> {
    let today = Timestamp::now().date_naive();
    let data = ctx.data.read().await;
    let daily_entry = data.get::<LuckymonDailyEntry>().unwrap();
    let entry = match daily_entry.get(&(user_id, today)) {
        Some(num) => Some(*num),
        None => None
    };

    entry
}

#[command]
#[description = "Lucky pokemon of the day!"]
async fn luckymon(ctx: &Context, msg: &Message) -> CommandResult {
    println!("Got luckymon command..");
    let user_id = msg.author.id;

    let mut lucky_num = fastrand::i64(1..=905);
    if let Some(num) = read_daily_entry(ctx, user_id).await {
        lucky_num = num;
    } else {
        write_daily_entry(ctx, user_id, lucky_num).await;
    }

    let rustemon_client = RustemonClient::default();
    let lucky_pokemon: Pokemon = pokemon::get_by_id(lucky_num, &rustemon_client).await?;
    let capitalized_name = lucky_pokemon.name[0..1].to_uppercase() + &lucky_pokemon.name[1..];

    let _msg = msg
        .channel_id
        .send_message(&ctx.http, |m| {
            m.embed(|e| {
                e.title("You lucky pokemon of the day is:")
                    .image(lucky_pokemon.sprites.front_default.unwrap().as_str())
                    .fields(vec!((format!("{}!", &capitalized_name), format!("[Bulbapedia Page](https://bulbapedia.bulbagarden.net/wiki/{}_(Pok%C3%A9mon))", capitalized_name).to_string(), false)))
                    .timestamp(Timestamp::now())
            })
        })
        .await;

    println!("Finished processing luckymon command!");
    Ok(())
}

