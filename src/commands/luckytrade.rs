use chrono::NaiveDate;
use image::{imageops, ImageBuffer, Rgba};
use rand::Rng;
use reqwest::header::CONTENT_TYPE;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serenity::builder::{CreateActionRow, CreateButton, CreateEmbed};
use serenity::model::prelude::component::ButtonStyle;
use serenity::model::prelude::InteractionResponseType;
use serenity::prelude::*;
use std::collections::HashMap;
use std::time::Duration;
use uuid::Uuid;

use super::image_host;
use crate::slash::Invocation;

/// How long a trade request stays open before its buttons come off.
const TRADE_TIMEOUT_SECS: u64 = 1200;

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

/// `/luckytrade` -- swap a luckymon with another player.
///
/// Discord guarantees all three options are present and that `user` really is a
/// user, so the arity and mention-parsing checks the prefix version needed are
/// gone. What is left is the part Discord cannot know: whether "123s" names a
/// pokemon, and whether both sides asked for nothing.
pub async fn slash_luckytrade(inv: &Invocation<'_>) -> serenity::Result<()> {
    inv.defer(false).await?;

    let ctx = inv.ctx;
    let caller = inv.user().clone();

    let callee_id = match inv.user_arg("user") {
        Some(id) => id,
        None => return inv.fail("Pick someone to trade with.").await,
    };

    if caller.id == callee_id {
        return inv.fail("You can't trade yourself, silly!").await;
    }

    let caller_side = match read_side(inv.string("offer")) {
        Ok(side) => side,
        Err(bad) => {
            return inv
                .fail(format!(
                    "`{}` isn't a pokedex number. Use `143`, or `143s` for a shiny \u{2014} or leave `offer` empty to ask for a gift.",
                    bad
                ))
                .await
        }
    };

    let callee_side = match read_side(inv.string("request")) {
        Ok(side) => side,
        Err(bad) => {
            return inv
                .fail(format!(
                    "`{}` isn't a pokedex number. Use `143`, or `143s` for a shiny \u{2014} or leave `request` empty to give a gift.",
                    bad
                ))
                .await
        }
    };

    if caller_side.is_none() && callee_side.is_none() {
        return inv
            .fail("A trade needs a pokemon on at least one side. Fill in `offer` to give one, `request` to ask for one, or both to swap.")
            .await;
    }

    let caller_na = caller_side.is_none();
    let callee_na = callee_side.is_none();
    let caller_luckymon = caller_side.unwrap_or_default();
    let callee_luckymon = callee_side.unwrap_or_default();

    // Check to see if each user has the requested pokemon.
    //
    // Reported rather than `?`d out: this command has already deferred, so
    // returning early here would leave "Bot is thinking..." spinning forever
    // with nothing on its way to replace it.
    let caller_hists = match untraded_collection(i64::from(caller.id)).await {
        Ok(hists) => hists,
        Err(e) => return inv.fail(e).await,
    };

    let callee_hists = match untraded_collection(i64::from(callee_id)).await {
        Ok(hists) => hists,
        Err(e) => {
            return inv
                .fail(format!("Couldn't read {}'s collection. {}", callee_id.mention(), e))
                .await
        }
    };

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
            return inv
                .fail(format!(
                    "You don't have a luckymon with ID {}!",
                    caller_luckymon
                ))
                .await;
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
            return inv
                .fail(format!(
                    "{} doesn't have a luckymon with ID {}!",
                    callee_id.mention(),
                    callee_luckymon
                ))
                .await;
        } else {
            callee_hist_id = callee_luckymon_hist.as_ref().unwrap().id;
            hist_data.push(callee_luckymon_hist.clone());
        }
    }

    // Hosted rather than attached: an interaction response cannot be edited to
    // carry a new attachment, and the accept/cancel edits that follow all need
    // the picture to stay put.
    let luckytrade_image = create_page_image(hist_data);
    let image_url = image_host::host(ctx, &luckytrade_image).await;

    // A one-sided exchange is a gift in one direction or the other, and saying
    // so beats "is offering their: n/a".
    let (title, blurb) = match (caller_na, callee_na) {
        (false, true) => (
            "\u{1F381} Luckymon Gift!",
            format!(
                "{} wants to give {} a pokemon, asking nothing back.",
                caller.mention(),
                callee_id.mention()
            ),
        ),
        (true, false) => (
            "\u{1F91D} Luckymon Request!",
            format!(
                "{} is asking {} for a pokemon, with nothing on offer.",
                caller.mention(),
                callee_id.mention()
            ),
        ),
        _ => (
            "Luckytrade Request!",
            format!(
                "{} has requested a trade with {}.",
                caller.mention(),
                callee_id.mention()
            ),
        ),
    };

    // Build the embedded message with images
    let embed = (*CreateEmbed::default()
        .title(title)
        .description(blurb)
        .field(
            "Their Offer",
            format!(
                "{} is offering: {}",
                caller.mention(),
                side_label(&caller_luckymon, caller_na)
            ),
            true,
        )
        .field("", "", true) // for spacing
        .field(
            "Requested Offer",
            format!(
                "{} wants: {}",
                caller.mention(),
                side_label(&callee_luckymon, callee_na)
            ),
            true,
        )
        .footer(|f| f.text("Click a button to respond.")))
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

    // The trade request *is* the reply, not a separate message posted next to
    // it. Sending it to the channel left the deferred response unfilled, so
    // every trade also left a permanent "Bot is thinking..." behind it.
    let mut embed = embed;
    embed.image(&image_url);

    inv.command
        .edit_original_interaction_response(&ctx.http, |r| {
            r.set_embed(embed);
            r.components(|c| c.add_action_row(action_row))
        })
        .await?;

    let msg = inv.command.get_interaction_response(&ctx.http).await?;

    while let Some(interaction) = msg
        .await_component_interaction(&ctx)
        .timeout(Duration::from_secs(TRADE_TIMEOUT_SECS))
        .await
    {
        if interaction.user.id != caller.id && interaction.user.id != callee_id {
            // Answer bystanders too, otherwise their click sits unacknowledged
            // and Discord shows them "the application didn't respond in time".
            let _ = interaction
                .create_interaction_response(&ctx.http, |r| {
                    r.kind(InteractionResponseType::ChannelMessageWithSource)
                        .interaction_response_data(|d| {
                            d.ephemeral(true)
                                .content("This trade isn't yours to answer.")
                        })
                })
                .await;
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

            // `!callee_na`, not `callee_na`. With the negation missing, the
            // second clause could only fire when the callee was giving nothing
            // -- exactly when `callee_hist` is None and the clause is dead. A
            // stale pokemon on their side therefore went undetected, which is
            // the duping this check exists to stop.
            if (!caller_hist.is_none() && !caller_na && caller_hist.as_ref().unwrap().traded)
                || (!callee_hist.is_none() && !callee_na && callee_hist.as_ref().unwrap().traded)
            {
                interaction
                    .edit_original_interaction_response(&ctx.http, |r| {
                        r.embed(|e| {
                            e.title("\u{274C} Trade Aborted! \u{274C}")
                                .description(
                                    "Luckymon data is outdated! Please create a new trade request.",
                                )
                                .image(&image_url)
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
                        e.title("\u{2705} Trade Accepted! \u{2705}")
                            .description(format!(
                                "{} has accepted the trade request from {}. \u{1F389}",
                                interaction.user.id.mention(),
                                caller.mention()
                            ))
                            .image(&image_url)
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
                        e.title("\u{274C} Trade Cancelled! \u{274C}")
                            .description(format!(
                                "{} has cancelled the trade request. \u{1F622}",
                                interaction.user.id.mention()
                            ))
                            .image(&image_url)
                    })
                })
                .await?;

            break;
        }
    }

    // Reached whether the trade completed, was cancelled, or timed out. In every
    // case the collector is gone, so the buttons have to go with it -- a
    // finished trade that still shows a live "Accept" is the worst of the three.
    //
    // Through the interaction endpoint, since the trade request is this
    // command's own reply rather than a message of its own.
    let _ = inv
        .command
        .edit_original_interaction_response(&ctx.http, |r| r.components(|c| c))
        .await;

    Ok(())
}

/// Everything a user still owns, traded-away entries excluded.
async fn untraded_collection(user_id: i64) -> Result<Vec<LuckymonHistory>, String> {
    let resp = reqwest::get(format!(
        "http://localhost:8000/api/v1/luckymon-history/user-id/{}",
        user_id
    ))
    .await
    .map_err(|e| format!("Could not reach the server: {}", e))?;

    let rows = resp
        .json::<Vec<HashMap<String, Value>>>()
        .await
        .map_err(|e| format!("Could not read the collection: {}", e))?;

    Ok(rows
        .into_iter()
        .map(LuckymonHistory::to_hist)
        .filter(|h| !h.traded)
        .collect())
}

/// Reads one side of a trade.
///
/// `Ok(None)` means "nothing from this side", which is what makes a gift a
/// gift. Omitting the option is the natural way to say that now that Discord
/// draws a form; a literal `n/a` is still accepted because that is what the old
/// prefix command took, and someone will type it out of habit.
///
/// `Err` carries the offending text back so the message can quote it.
fn read_side(raw: Option<String>) -> Result<Option<String>, String> {
    let value = match raw {
        Some(v) => v.trim().to_string(),
        None => return Ok(None),
    };

    if value.is_empty() || value.eq_ignore_ascii_case("n/a") {
        return Ok(None);
    }

    if validate_trade_arg(&value) {
        Ok(Some(value))
    } else {
        Err(value)
    }
}

/// How a side reads in the embed.
fn side_label(value: &str, is_gift: bool) -> String {
    if is_gift {
        "nothing \u{2014} it's a gift".to_string()
    } else {
        format!("`{}`", value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Leaving a side out is what makes a gift. This is the behaviour that was
    /// unreachable while both options were required.
    #[test]
    fn an_omitted_side_is_a_gift() {
        assert_eq!(read_side(None), Ok(None));
        assert_eq!(read_side(Some("".to_string())), Ok(None));
        assert_eq!(read_side(Some("   ".to_string())), Ok(None));
    }

    /// The prefix command's spelling still works, because people will type it.
    #[test]
    fn na_is_still_accepted() {
        assert_eq!(read_side(Some("n/a".to_string())), Ok(None));
        assert_eq!(read_side(Some("N/A".to_string())), Ok(None));
        assert_eq!(read_side(Some(" n/A ".to_string())), Ok(None));
    }

    #[test]
    fn a_pokemon_comes_back_intact() {
        assert_eq!(read_side(Some("143".to_string())), Ok(Some("143".to_string())));
        assert_eq!(
            read_side(Some("143s".to_string())),
            Ok(Some("143s".to_string()))
        );
        // Surrounding spaces should not make a valid entry fail.
        assert_eq!(read_side(Some(" 6 ".to_string())), Ok(Some("6".to_string())));
    }

    #[test]
    fn nonsense_comes_back_for_quoting() {
        assert_eq!(
            read_side(Some("snorlax".to_string())),
            Err("snorlax".to_string())
        );
        assert_eq!(read_side(Some("12x".to_string())), Err("12x".to_string()));
    }

    #[test]
    fn gift_sides_are_labelled_rather_than_left_blank() {
        assert_eq!(side_label("", true), "nothing \u{2014} it's a gift");
        assert_eq!(side_label("143", false), "`143`");
    }
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
