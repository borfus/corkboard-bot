use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::hash::{Hash, Hasher};
use std::io::copy;
use std::path::Path;

use chrono::NaiveDate;
use rustemon::client::RustemonClient;
use rustemon::model::pokemon::Pokemon;
use rustemon::pokemon::pokemon;
use serde::Serialize;
use serde_json::Value;
use serenity::framework::standard::macros::command;
use serenity::framework::standard::CommandResult;
use serenity::futures::StreamExt;
use serenity::model::channel::Message;
use serenity::model::Timestamp;
use serenity::prelude::*;

extern crate reqwest;
extern crate tokio;

static POKEDEX_MAX_NUM: u64 = 1010;

#[derive(Serialize, Debug)]
pub struct NewLuckymonHistory {
    pub user_id: i64,
    pub date_obtained: NaiveDate,
    pub pokemon_id: i64,
    pub shiny: bool,
    pub pokemon_name: String,
}

impl NewLuckymonHistory {
    pub fn new(
        user_id: i64,
        date_obtained: NaiveDate,
        pokemon_id: i64,
        shiny: bool,
        pokemon_name: &String,
    ) -> Self {
        NewLuckymonHistory {
            user_id,
            date_obtained,
            pokemon_id,
            shiny,
            pokemon_name: pokemon_name.to_string(),
        }
    }
}

fn calculate_hash<T: Hash, U: Hash>(t: &T, u: &U) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    u.hash(&mut s);
    s.finish()
}

fn capitalize(name: &str) -> String {
    let (n1, n2) = name.split_at(1);
    let n_upper = n1.to_uppercase();
    let new_name = n_upper + n2;
    return new_name.to_string();
}

fn capitalize_hyphenated(name: &str, separator: &str) -> String {
    let hyphen_index = name.chars().position(|c| c == '-').unwrap();
    let (n1, eh) = name.split_at(hyphen_index);
    let (_uh, n2) = eh.split_at(1);
    let new_name = capitalize(n1) + separator + &capitalize(n2);
    return new_name.to_string();
}

fn has_hyphen(name: &str) -> bool {
    return name.contains("-");
}

fn is_nidoran(name: &str) -> bool {
    return name.contains("idoran");
}

fn is_paradox(name: &str) -> bool {
    return name.starts_with("iron-")
        || name.starts_with("scream-")
        || name.starts_with("slither-")
        || name.starts_with("brute-")
        || name.starts_with("great-")
        || name.starts_with("flutter-")
        || name.starts_with("sandy-");
}

fn format_for_display(name: &str) -> String {
    // this includes pokemon with hyphenated names as well as pokemon who have spaces in their names
    if has_hyphen(name) {
        if is_nidoran(name) {
            // nidoran male and female need special display
            if name.contains("-f") {
                return "Nidoran \\♀".to_string();
            } else {
                return "Nidoran \\♂".to_string();
            }
        }

        // jangmo-o, hakamo-o, kommo-o don't need changing
        if name.ends_with("-o") {
            return capitalize(name).to_string();
        }

        if name.ends_with("-oh") || name.ends_with("-z") {
            return capitalize_hyphenated(name, "-");
        }

        if name.ends_with("-mime") {
            return "Mr. Mime".to_string();
        }

        if name.eq("type-null") {
            return "Type: Null".to_string();
        }

        if is_paradox(name) || name.starts_with("tapu") {
            return capitalize_hyphenated(name, " ");
        }

        // other stuff falls thru and removes the hyphen
        return capitalize_hyphenated(name, " ");
    }

    // fall through when no hyphen and default just to capitalizing the string once at the start;
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

        if name.ends_with("-oh") || name.ends_with("-z") {
            return capitalize_hyphenated(name, "-");
        }

        if name.ends_with("-mime") {
            return "Mr._Mime".to_string();
        }

        if name.eq("type-null") {
            return "Type:_Null".to_string();
        }

        // strip hyphens from the Tapu* pokemon and replace with a space + capitalize
        if name.starts_with("tapu") {
            return capitalize_hyphenated(name, "_");
        }

        if is_paradox(name) {
            return capitalize_hyphenated(name, "_");
        }
    }
    return capitalize(name).to_string();
}

#[command]
#[description = "Lucky pokemon of the day!"]
async fn luckymon(ctx: &Context, msg: &Message) -> CommandResult {
    println!("Got luckymon command..");
    let user_id = msg.author.id;
    let today = Timestamp::now().date_naive();

    let one_in_x_shiny_chance = 400; // 1/400 chance to get a shiny
    let user_hash = calculate_hash(&user_id, &today);
    let lucky_num = user_hash % POKEDEX_MAX_NUM + 1;
    let shiny_num = (user_hash >> 10) % one_in_x_shiny_chance + 1;

    let mut is_shiny = false;
    if shiny_num == 1 {
        is_shiny = true;
    }

    let daily_pair: (i64, bool) = (lucky_num.try_into().unwrap(), is_shiny);

    println!(
        "User ID {} ran luckymon command!: Got number {} and shiny {}",
        user_id, daily_pair.0, daily_pair.1
    );
    println!("user_hash: {}", user_hash);
    println!(
        "Luckymon daily_pair: {} - {} - {:?}",
        daily_pair.0, daily_pair.1, today
    );

    let rustemon_client = RustemonClient::default();
    let lucky_pokemon: Pokemon = pokemon::get_by_id(daily_pair.0, &rustemon_client).await?;

    let regular_name = lucky_pokemon.species.name;
    let display_name = format_for_display(&regular_name);
    let mut final_name = String::from(display_name.clone());
    let link_name = format_for_bulba(&regular_name);
    let regular_sprite = lucky_pokemon.sprites.front_default.unwrap();

    let mut sprite = regular_sprite;

    let new = NewLuckymonHistory::new(
        i64::from(user_id),
        today,
        daily_pair.0,
        daily_pair.1,
        &display_name,
    );

    if daily_pair.1 {
        if let Some(shiny_sprite) = lucky_pokemon.sprites.front_shiny {
            final_name = format!("✨ Shiny {} ✨", final_name);
            sprite = shiny_sprite;
        }
    }

    println!(
        "Sending new LuckymonHistory creation request with {:?}",
        new
    );
    let client = reqwest::Client::new();
    let _resp = client
        .post("http://localhost:8000/api/v1/luckymon-history")
        .json(&new)
        .send()
        .await?
        .json::<HashMap<String, Value>>()
        .await?;

    let author_name = &msg.author.name.clone();
    let avatar_url = &msg.author.avatar_url().unwrap().clone();
    let _msg = msg
        .channel_id
        .send_message(&ctx.http, |m| {
            m.embed(|e| {
                e.title("Your lucky Pokémon of the day is:")
                    .image(sprite)
                    .fields(vec!((format!("{}", &final_name), format!("[Bulbapedia Page](https://bulbapedia.bulbagarden.net/wiki/{}_(Pok%C3%A9mon))", link_name).to_string(), false)))
                    .footer(|f| {
                        f.text(format!("{} - Resets 5PM PDT (12AM UTC)", author_name));
                        f.icon_url(avatar_url)
                    })
            })
        })
        .await;

    println!("Finished processing luckymon command!");
    Ok(())
}

pub async fn initialize() {
    println!("Begin initialization for luckymon.");
    download_sprites().await;
    println!("Initialization complete!");
}

pub async fn download_sprites() {
    let path = "./resources/sprites";
    let _ = fs::create_dir_all(path);
    let rustemon_client = RustemonClient::default();

    println!("Downloading sprites...");
    for i in 1..=POKEDEX_MAX_NUM {
        let pokemon: Pokemon = pokemon::get_by_id(i.try_into().unwrap(), &rustemon_client)
            .await
            .unwrap();

        download_individual_sprite(
            pokemon.sprites.front_default.unwrap(),
            format!("{}/{}.png", path, i),
        )
        .await;
        if let Some(shiny_url) = pokemon.sprites.front_shiny {
            download_individual_sprite(shiny_url, format!("{}/{}_shiny.png", path, i)).await;
        }
    }
    println!("Done!");
}

async fn download_individual_sprite(url: String, file_name: String) {
    if !Path::new(&file_name).exists() {
        let response = reqwest::get(url).await.unwrap();
        if response.status().is_success() {
            let mut file_shiny = File::create(Path::new(&file_name)).unwrap();
            let mut stream = response.bytes_stream();
            while let Some(item) = stream.next().await {
                let chunk = item.unwrap();
                let _ = copy(&mut chunk.as_ref(), &mut file_shiny);
            }
        }
    } else {
        println!("{} exists. Skipping.", file_name);
    }
}
