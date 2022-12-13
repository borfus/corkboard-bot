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
    type Value = HashMap<(UserId, NaiveDate), (i64, bool)>;
}

async fn write_daily_entry(ctx: &Context, user_id: UserId, lucky_num: i64, shiny: bool) {
    let today = Timestamp::now().date_naive();
    let mut data = ctx.data.write().await;
    let daily_entry = data.get_mut::<LuckymonDailyEntry>().unwrap();
    let entry = daily_entry.entry((user_id, today)).or_insert((lucky_num, shiny));
    *entry = (lucky_num, shiny);
}

async fn read_daily_entry(ctx: &Context, user_id: UserId) -> Option<(i64, bool)> {
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

    let lucky_num = fastrand::i64(1..=905);
    let shiny_num = fastrand::i64(1..=500);
    let mut is_shiny = false;
    if shiny_num == 1 {
        is_shiny = true;
    }

    let mut daily_pair: (i64, bool) = (lucky_num, is_shiny);

    if let Some((num, shiny)) = read_daily_entry(ctx, user_id).await {
        daily_pair = (num, shiny);
    } else {
        write_daily_entry(ctx, user_id, lucky_num, is_shiny).await;
    }

    let rustemon_client = RustemonClient::default();
    let lucky_pokemon: Pokemon = pokemon::get_by_id(lucky_num, &rustemon_client).await?;

    let regular_name = lucky_pokemon.name[0..1].to_uppercase() + &lucky_pokemon.name[1..];
    let mut final_name = String::from(&regular_name);
    let regular_sprite = lucky_pokemon.sprites.front_default.unwrap();
    let shiny_sprite = lucky_pokemon.sprites.front_shiny.unwrap();
    let mut sprite = regular_sprite;
    if daily_pair.1 {
        final_name = format!("Shiny {}", regular_name);
        sprite = shiny_sprite;
    }

    let _msg = msg
        .channel_id
        .send_message(&ctx.http, |m| {
            m.embed(|e| {
                e.title("Your lucky pokemon of the day is:")
                    .image(sprite)
                    .fields(vec!((format!("{}!", &final_name), format!("[Bulbapedia Page](https://bulbapedia.bulbagarden.net/wiki/{}_(Pok%C3%A9mon))", regular_name).to_string(), false)))
                    .timestamp(Timestamp::now())
            })
        })
        .await;

    println!("Finished processing luckymon command!");
    Ok(())
}

