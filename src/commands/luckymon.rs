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

fn capitalize(name: &str) -> String {
    let (n1, n2) = name.split_at(1);
    let n_upper = n1.to_uppercase();
    let new_name = n_upper + n2;
    return new_name.to_string();
}

fn capitalize_hyphenated(name: &str, keep_hyphen: bool) -> String {
    let hyphen_index = name.chars().position(|c| c == '-').unwrap();
    let (n1, eh) = name.split_at(hyphen_index);
    let (_uh, n2) = eh.split_at(1);
    let middle_char = if keep_hyphen { "-" } else { " " };
    let new_name = capitalize(n1) + middle_char + &capitalize(n2);
    return new_name.to_string();
}


fn has_hyphen(name: &str) -> bool {
    return name.contains("-");
}

fn is_nidoran(name: &str) -> bool {
    return name.contains("idoran"); // sorry lol
}


fn format_for_display(name: &str) -> String {
    if has_hyphen(name) {
        if is_nidoran(name) {
            // nidoran male and female need special display
            if name.contains("-f") {
                return "Nidoran \\♀".to_string();
            } else {
                return "Nidoran \\♂".to_string();
            }
        }

        if name.contains("-oh") || name.contains("-z") {
            return capitalize_hyphenated(name, true);
        }

        // other stuff falls thru and removes the hyphen
        return capitalize_hyphenated(name, false);
    }
    return capitalize(name).to_string();
}


fn format_for_bulba(name: &str) -> String {
    if has_hyphen(name) {
        // nidoran male and female need encoding
        if is_nidoran(name) {
            if name.contains("-f") {
                return "Nidoran%E2%99%80".to_string();
            } else {
                return "Nidoran%E2%99%82".to_string();
            }
        }

        if name.contains("-oh") || name.contains("-z") {
            return capitalize_hyphenated(name, true);
        }
    }
    return capitalize(name).to_string();
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
    println!("User ID {} ran luckymon command!: Got number {} and shiny {}", user_id, daily_pair.0, daily_pair.1);
    let today = Timestamp::now().date_naive();
    println!("Luckymon daily_pair: {} - {} - {:?}", daily_pair.0, daily_pair.1, today);

    let rustemon_client = RustemonClient::default();
    let lucky_pokemon: Pokemon = pokemon::get_by_id(daily_pair.0, &rustemon_client).await?;

    let regular_name = lucky_pokemon.species.name;
    let display_name = format_for_display(&regular_name);
    let mut final_name = String::from(display_name);
    let link_name = format_for_bulba(&regular_name);
    let regular_sprite = lucky_pokemon.sprites.front_default.unwrap();

    let mut sprite = regular_sprite;
    if daily_pair.1 {
        final_name = format!("Shiny {}", regular_name);
        sprite = lucky_pokemon.sprites.front_shiny.unwrap()
    }

    let _msg = msg
        .channel_id
        .send_message(&ctx.http, |m| {
            m.embed(|e| {
                e.title("Your lucky pokemon of the day is:")
                    .image(sprite)
                    .fields(vec!((format!("{}!", &final_name), format!("[Bulbapedia Page](https://bulbapedia.bulbagarden.net/wiki/{}_(Pok%C3%A9mon))", link_name).to_string(), false)))
                    .footer(|f| f.text("Resets daily at 4PM Pacific Time (12AM UTC)"))
                    .timestamp(Timestamp::now())
            })
        })
        .await;

    println!("Finished processing luckymon command!");
    Ok(())
}

