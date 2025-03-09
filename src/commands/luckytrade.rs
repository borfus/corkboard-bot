use chrono::NaiveDate;
use image::codecs::png::PngEncoder;
use image::{imageops, ImageBuffer, ImageEncoder, Rgba};
use rand::Rng;
use reqwest::header::CONTENT_TYPE;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serenity::builder::{CreateActionRow, CreateButton, CreateEmbed};
use serenity::framework::standard::macros::command;
use serenity::framework::standard::{Args, CommandError, CommandResult};
use serenity::model::channel::Message;
use serenity::model::id::UserId;
use serenity::model::prelude::component::ButtonStyle;
use serenity::model::prelude::{AttachmentType, InteractionResponseType};
use serenity::prelude::*;
use serenity::utils::parse_username;
use std::collections::HashMap;
use std::io::Cursor;
use std::time::Duration;
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LuckymonHistory {
    pub id: Uuid,
    pub user_id: i64,
    pub date_obtained: NaiveDate,
    pub pokemon_id: i64,
    pub shiny: bool,
    pub pokemon_name: String,
    pub traded: bool,
}

impl LuckymonHistory {
    pub fn new(
        id: &str,
        user_id: i64,
        date_obtained: &str,
        pokemon_id: i64,
        shiny: bool,
        pokemon_name: String,
        traded: bool,
    ) -> Self {
        let id = Uuid::parse_str(id).expect("Bad UUID");

        let fmt = "%Y-%m-%d";
        let date_obtained = NaiveDate::parse_from_str(date_obtained, fmt)
            .expect("Unable to parse date_obtained NaiveDate for LuckymonHistory.");

        LuckymonHistory {
            id,
            user_id,
            date_obtained,
            pokemon_id,
            shiny,
            pokemon_name,
            traded,
        }
    }

    pub fn to_hist(hist_map: HashMap<String, Value>) -> Self {
        LuckymonHistory::new(
            hist_map.get("id").unwrap().as_str().unwrap(),
            hist_map.get("user_id").unwrap().as_i64().unwrap(),
            hist_map.get("date_obtained").unwrap().as_str().unwrap(),
            hist_map.get("pokemon_id").unwrap().as_i64().unwrap(),
            hist_map.get("shiny").unwrap().as_bool().unwrap(),
            hist_map
                .get("pokemon_name")
                .unwrap()
                .as_str()
                .unwrap()
                .to_string(),
            hist_map.get("traded").unwrap().as_bool().unwrap(),
        )
    }
}

#[derive(Serialize, Debug)]
pub struct NewLuckymonHistory {
    pub user_id: i64,
    pub date_obtained: NaiveDate,
    pub pokemon_id: i64,
    pub shiny: bool,
    pub pokemon_name: String,
    pub traded: bool,
}

impl NewLuckymonHistory {
    pub fn new(
        user_id: i64,
        date_obtained: NaiveDate,
        pokemon_id: i64,
        shiny: bool,
        pokemon_name: &String,
        traded: bool,
    ) -> Self {
        NewLuckymonHistory {
            user_id,
            date_obtained,
            pokemon_id,
            shiny,
            pokemon_name: pokemon_name.to_string(),
            traded,
        }
    }
}

#[command]
#[description = "Trade Your Luckymon With Other Users."]
async fn luckytrade(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    println!("Got luckytrade command..");

    let caller = &msg.author;

    // Error handling: check if we have the correct number of arguments
    if args.len() < 3 {
        msg.channel_id
            .say(
                &ctx.http,
                format!(
                    "{} Error: Not enough arguments. Usage: `.luckytrade @username 123 456s`
- Argument 1: User you wish to trade
- Argument 2: The Pokémon you wish to trade. Options are #, #s (With a trailing 's' to indicate shiny), or n/a (useful for gifting). Examples: 123, 123s, n/a
- Argument 3: The Pokémon you wish to receive from the person you pinged (Argument 1).", 
                    caller
                ),
            )
            .await?;
        return Err(CommandError::from("Not enough arguments."));
    }

    // Extract and validate the user mention
    let mention = args.single::<String>()?;
    let callee_id = match parse_username(&mention) {
        Some(id) => UserId(id),
        None => {
            msg.channel_id
                .say(
                    &ctx.http,
                    format!("{} Error: Invalid user mention.", caller),
                )
                .await?;
            return Err(CommandError::from("Invalid user mention."));
        }
    };

    if caller.id == callee_id {
        msg.channel_id
            .say(
                &ctx.http,
                format!("{} Error: You can't trade yourself, silly!", caller),
            )
            .await?;
        return Err(CommandError::from("Tried trading themselves."));
    }

    // Extract and validate the next 2 luckymon trade arguments
    let caller_luckymon = args.single::<String>()?;
    if !validate_trade_arg(&caller_luckymon) {
        msg.channel_id
            .say(
                &ctx.http,
                format!(
                    "{} Error: Invalid format for the first trade argument.",
                    caller
                ),
            )
            .await?;
        return Err(CommandError::from(
            "Invalid format for the first trade argument.",
        ));
    }

    let callee_luckymon = args.single::<String>()?;
    if !validate_trade_arg(&callee_luckymon) {
        msg.channel_id
            .say(
                &ctx.http,
                format!(
                    "{} Error: Invalid format for the second trade argument.",
                    caller
                ),
            )
            .await?;
        return Err(CommandError::from(
            "Invalid format for the second trade argument.",
        ));
    }

    let caller_na = caller_luckymon.eq_ignore_ascii_case("n/a");
    let callee_na = callee_luckymon.eq_ignore_ascii_case("n/a");
    if caller_na && callee_na {
        msg.channel_id
            .say(
                &ctx.http,
                format!("{} Error: Both luckymon can't be 'N/A'.", caller),
            )
            .await?;
        return Err(CommandError::from("Invalid arguments provided."));
    }

    // Check to see if each user has the requested pokemon
    let resp = reqwest::get(format!(
        "http://localhost:8000/api/v1/luckymon-history/user-id/{}",
        i64::from(msg.author.id)
    ))
    .await?
    .json::<Vec<HashMap<String, Value>>>()
    .await?;
    let mut caller_hists: Vec<LuckymonHistory> = Vec::new();
    for hist_map in resp {
        let hist = LuckymonHistory::to_hist(hist_map);
        if !hist.traded {
            caller_hists.push(hist);
        }
    }

    let resp = reqwest::get(format!(
        "http://localhost:8000/api/v1/luckymon-history/user-id/{}",
        i64::from(callee_id)
    ))
    .await?
    .json::<Vec<HashMap<String, Value>>>()
    .await?;
    let mut callee_hists: Vec<LuckymonHistory> = Vec::new();
    for hist_map in resp {
        let hist = LuckymonHistory::to_hist(hist_map);
        if !hist.traded {
            callee_hists.push(hist);
        }
    }

    let caller_shiny = caller_luckymon.ends_with("s");
    let callee_shiny = callee_luckymon.ends_with("s");

    let caller_luckymon_id;
    let callee_luckymon_id;

    let mut hist_data = Vec::new();

    let mut caller_hist_id = Uuid::new_v4();
    if caller_na {
        hist_data.push(None);
    } else {
        if caller_shiny {
            let digits: String = caller_luckymon
                .chars()
                .take_while(|c| c.is_digit(10))
                .collect();
            caller_luckymon_id = digits.parse::<i64>().unwrap();
        } else {
            caller_luckymon_id = caller_luckymon.parse::<i64>().unwrap();
        }

        let caller_luckymon_hist = caller_hists
            .into_iter()
            .find(|h| h.pokemon_id == caller_luckymon_id);

        if caller_luckymon_hist.is_none() {
            msg.channel_id
                .say(
                    &ctx.http,
                    format!(
                        "{} Error: You don't have a luckymon with ID {}!",
                        caller, caller_luckymon
                    ),
                )
                .await?;
            return Err(CommandError::from("Caller doesn't have this luckymon."));
        } else {
            caller_hist_id = caller_luckymon_hist.as_ref().unwrap().id;
            hist_data.push(caller_luckymon_hist);
        }
    }

    let mut callee_hist_id = Uuid::new_v4();
    if callee_na {
        hist_data.push(None);
    } else {
        if callee_shiny {
            let digits: String = callee_luckymon
                .chars()
                .take_while(|c| c.is_digit(10))
                .collect();
            callee_luckymon_id = digits.parse::<i64>().unwrap();
        } else {
            callee_luckymon_id = callee_luckymon.parse::<i64>().unwrap();
        }

        let callee_luckymon_hist = callee_hists
            .into_iter()
            .find(|h| h.pokemon_id == callee_luckymon_id);

        if callee_luckymon_hist.is_none() {
            msg.channel_id
                .say(
                    &ctx.http,
                    format!(
                        "{} Error: {} doesn't have a luckymon with ID {}!",
                        caller,
                        callee_id.mention(),
                        callee_luckymon
                    ),
                )
                .await?;
            return Err(CommandError::from("Callee doesn't have this luckymon."));
        } else {
            callee_hist_id = callee_luckymon_hist.as_ref().unwrap().id;
            hist_data.push(callee_luckymon_hist.clone());
        }
    }

    let luckytrade_image = create_page_image(hist_data);
    let mut buffer: Vec<u8> = Vec::new();
    {
        let mut writer = Cursor::new(&mut buffer);
        let encoder = PngEncoder::new(&mut writer);
        encoder
            .write_image(
                &luckytrade_image,
                luckytrade_image.width(),
                luckytrade_image.height(),
                image::ColorType::Rgba8,
            )
            .expect("Error encoding image");
    }

    let files = vec![AttachmentType::Bytes {
        data: buffer.into(),
        filename: "image.png".to_string(),
    }];

    // Build the embedded message with images
    let embed = (*CreateEmbed::default()
        .title("Luckytrade Request!")
        .description(format!(
            "{} has requested a trade with {}.",
            caller.mention(),
            callee_id.mention()
        ))
        .field(
            "Their Offer",
            format!(
                "{} is offering their: {}",
                caller.mention(),
                caller_luckymon
            ),
            true,
        )
        .field("", "", true) // for spacing
        .field(
            "Requested Offer",
            format!("{} wants your: {}", caller.mention(), callee_luckymon),
            true,
        )
        .footer(|f| f.text("Click a button to respond to the trade.")))
    .clone();

    // Adding buttons for trade acceptance or cancel
    let action_row = (*CreateActionRow::default()
        .add_button(
            (*CreateButton::default()
                .custom_id("accept_trade")
                .label("Accept")
                .style(ButtonStyle::Success))
            .clone(),
        )
        .add_button(
            (*CreateButton::default()
                .custom_id("cancel_trade")
                .label("Cancel")
                .style(ButtonStyle::Danger))
            .clone(),
        ))
    .clone();

    // Send the embedded message with buttons
    let msg = msg
        .channel_id
        .send_message(&ctx.http, |m| {
            m.files(files)
                .embed(|e| {
                    *e = embed;
                    e.image(format!("attachment://image.png"));
                    e
                })
                .components(|c| c.add_action_row(action_row))
        })
        .await
        .unwrap();

    while let Some(interaction) = msg
        .await_component_interaction(&ctx)
        .timeout(Duration::from_secs(1200))
        .await
    {
        if interaction.user.id != caller.id && interaction.user.id != callee_id {
            continue;
        }

        interaction
            .create_interaction_response(&ctx.http, |r| {
                r.kind(InteractionResponseType::DeferredUpdateMessage)
            })
            .await?;

        let custom_id = &interaction.data.custom_id;
        if custom_id == "accept_trade" && interaction.user.id == callee_id {
            // Check to see if these luckymon history records still exist as they were
            // when the trade request was first created. This is to prevent duping!
            let mut caller_hist = None;
            if !caller_na {
                let resp = reqwest::get(format!(
                    "http://localhost:8000/api/v1/luckymon-history/{}",
                    caller_hist_id
                ))
                .await?
                .json::<HashMap<String, Value>>()
                .await?;
                caller_hist = Some(LuckymonHistory::to_hist(resp));
            }

            let mut callee_hist = None;
            if !callee_na {
                let resp = reqwest::get(format!(
                    "http://localhost:8000/api/v1/luckymon-history/{}",
                    callee_hist_id
                ))
                .await?
                .json::<HashMap<String, Value>>()
                .await?;
                callee_hist = Some(LuckymonHistory::to_hist(resp));
            }

            if (!caller_hist.is_none() && !caller_na && caller_hist.as_ref().unwrap().traded)
                || (!callee_hist.is_none() && callee_na && callee_hist.as_ref().unwrap().traded)
            {
                interaction
                    .edit_original_interaction_response(&ctx.http, |r| {
                        r.embed(|e| {
                            e.title("❌ Trade Aborted! ❌")
                                .description(
                                    "Luckymon data is outdated! Please create a new trade request.",
                                )
                                .image(format!("attachment://image.png"))
                        })
                    })
                    .await?;

                break;
            }

            if !callee_na {
                let callee_hist = callee_hist.unwrap();

                let new_caller_luckymon = NewLuckymonHistory::new(
                    caller.id.into(),
                    callee_hist.date_obtained,
                    callee_hist.pokemon_id,
                    callee_hist.shiny,
                    &callee_hist.pokemon_name,
                    false,
                );

                println!(
                    "Sending new LuckymonHistory creation request via trade with {:?}",
                    new_caller_luckymon
                );
                let client = reqwest::Client::new();
                let _resp = client
                    .post("http://localhost:8000/api/v1/luckymon-history?trade=true")
                    .json(&new_caller_luckymon)
                    .send()
                    .await?
                    .json::<HashMap<String, Value>>()
                    .await?;

                let client = reqwest::Client::new();
                let _resp = client
                    .put(format!(
                        "http://localhost:8000/api/v1/luckymon-history/traded/{}",
                        callee_hist_id
                    ))
                    .header(CONTENT_TYPE, "application/json")
                    .send()
                    .await?
                    .json::<HashMap<String, Value>>()
                    .await?;
            }

            if !caller_na {
                let caller_hist = caller_hist.unwrap();

                let new_callee_luckymon = NewLuckymonHistory::new(
                    callee_id.into(),
                    caller_hist.date_obtained,
                    caller_hist.pokemon_id,
                    caller_hist.shiny,
                    &caller_hist.pokemon_name,
                    false,
                );

                println!(
                    "Sending new LuckymonHistory creation request via trade with {:?}",
                    new_callee_luckymon
                );
                let client = reqwest::Client::new();
                let _resp = client
                    .post("http://localhost:8000/api/v1/luckymon-history?trade=true")
                    .json(&new_callee_luckymon)
                    .send()
                    .await?
                    .json::<HashMap<String, Value>>()
                    .await?;

                let client = reqwest::Client::new();
                let _resp = client
                    .put(format!(
                        "http://localhost:8000/api/v1/luckymon-history/traded/{}",
                        caller_hist_id
                    ))
                    .header(CONTENT_TYPE, "application/json")
                    .send()
                    .await?
                    .json::<HashMap<String, Value>>()
                    .await?;
            }

            interaction
                .edit_original_interaction_response(&ctx.http, |r| {
                    r.embed(|e| {
                        e.title("✅ Trade Accepted! ✅")
                            .description(format!(
                                "{} has accepted the trade request from {}. 🎉",
                                interaction.user.id.mention(),
                                caller.mention()
                            ))
                            .image(format!("attachment://image.png"))
                    })
                })
                .await?;

            break;
        } else if custom_id == "cancel_trade"
            && (interaction.user.id == caller.id || interaction.user.id == callee_id)
        {
            interaction
                .edit_original_interaction_response(&ctx.http, |r| {
                    r.embed(|e| {
                        e.title("❌ Trade Cancelled! ❌")
                            .description(format!(
                                "{} has cancelled the trade request. 😢",
                                interaction.user.id.mention()
                            ))
                            .image(format!("attachment://image.png"))
                    })
                })
                .await?;

            break;
        }
    }

    Ok(())
}

fn validate_trade_arg(arg: &str) -> bool {
    // Check if the argument is a number, a number followed by 's', or 'n/a'
    arg.parse::<i32>().is_ok()
        || arg.ends_with('s') && arg[..arg.len() - 1].parse::<i32>().is_ok()
        || arg.eq_ignore_ascii_case("n/a")
}

fn create_page_image(data: Vec<Option<LuckymonHistory>>) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
    let bg_root_path = "./resources/luckydex/";
    let sprite_root_path = "./resources/sprites/";
    let trade_sprite_path = "./resources/luckytrade/";
    let bg_dimension_x = 400;
    let bg_dimension_y = 150;
    let sprite_dimensions = 96; // all sprites are 96x96
    let grid_dimensions = 3; // num columns
    let background_filename = format!("bg{}.png", rand::thread_rng().gen_range(1..=20)); // 1 - 20

    let y_spacing_buffer = sprite_dimensions - 65;
    let x_spacing_buffer = (bg_dimension_x / grid_dimensions) / 2;

    // Calculate spacing
    let sprite_spacing =
        (bg_dimension_x - (grid_dimensions * sprite_dimensions)) / grid_dimensions - 15;

    let mut img = ImageBuffer::new(bg_dimension_x, bg_dimension_y);
    let background = image::open(format!("{}{}", bg_root_path, background_filename))
        .unwrap()
        .to_rgba8();

    imageops::overlay(&mut img, &background, 0, 0);

    let pokemon_data_1_maybe: &Option<LuckymonHistory> = data.get(0).unwrap();
    let pokemon_data_2_maybe: &Option<LuckymonHistory> = data.get(1).unwrap();

    let mut pokemon_sprite_1;
    if pokemon_data_1_maybe.is_none() {
        pokemon_sprite_1 = image::open(format!("{}na.png", trade_sprite_path))
            .unwrap()
            .to_rgba8();
    } else {
        let pokemon_data_1 = pokemon_data_1_maybe.as_ref().unwrap();
        pokemon_sprite_1 = image::open(format!(
            "{}{}.png",
            sprite_root_path, &pokemon_data_1.pokemon_id
        ))
        .unwrap()
        .to_rgba8();

        if pokemon_data_1.shiny {
            pokemon_sprite_1 = image::open(format!(
                "{}{}_shiny.png",
                sprite_root_path, &pokemon_data_1.pokemon_id
            ))
            .unwrap()
            .to_rgba8();
        }
    }

    let mut pokemon_sprite_2;
    if pokemon_data_2_maybe.is_none() {
        pokemon_sprite_2 = image::open(format!("{}na.png", trade_sprite_path))
            .unwrap()
            .to_rgba8();
    } else {
        let pokemon_data_2 = pokemon_data_2_maybe.as_ref().unwrap();
        pokemon_sprite_2 = image::open(format!(
            "{}{}.png",
            sprite_root_path, &pokemon_data_2.pokemon_id
        ))
        .unwrap()
        .to_rgba8();

        if pokemon_data_2.shiny {
            pokemon_sprite_2 = image::open(format!(
                "{}{}_shiny.png",
                sprite_root_path, &pokemon_data_2.pokemon_id
            ))
            .unwrap()
            .to_rgba8();
        }
    }

    let trade_sprite = image::open(format!("{}trade.png", trade_sprite_path))
        .unwrap()
        .to_rgba8();

    let mut x: i64 = (((0 * (sprite_dimensions + sprite_spacing)) + sprite_dimensions)
        - x_spacing_buffer)
        .into();
    let y: i64 = y_spacing_buffer.into();
    imageops::overlay(&mut img, &pokemon_sprite_1, x, y);

    x = (((1 * (sprite_dimensions + sprite_spacing)) + sprite_dimensions) - x_spacing_buffer)
        .into();
    imageops::overlay(&mut img, &trade_sprite, x, y);

    x = (((2 * (sprite_dimensions + sprite_spacing)) + sprite_dimensions) - x_spacing_buffer)
        .into();
    imageops::overlay(&mut img, &pokemon_sprite_2, x, y);

    return img;
}
