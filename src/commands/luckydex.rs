use std::collections::HashMap;
use std::time::Duration;

use serenity::prelude::*;
use serenity::model::channel::Message;
use serenity::framework::standard::macros::command;
use serenity::framework::standard::CommandResult;
use serenity::model::Timestamp;
use serenity::utils::Colour;
use serenity::model::application::component::ButtonStyle;
use serenity::builder::{CreateActionRow, CreateComponents};
use serenity::model::application::interaction::InteractionResponseType;

use serde::{Serialize, Deserialize};
use serde_json::Value;
use uuid::Uuid;
use chrono::NaiveDate;

#[derive(Serialize, Deserialize, Debug)]
pub struct LuckymonHistory {
    pub id: Uuid,
    pub user_id: i64,
    pub date_obtained: NaiveDate,
    pub pokemon_id: i64,
    pub shiny: bool,
    pub pokemon_name: String
}

impl LuckymonHistory {
    pub fn new(
        id: &str,
        user_id: i64,
        date_obtained: &str,
        pokemon_id: i64,
        shiny: bool,
        pokemon_name: String
    ) -> Self {
        let id = Uuid::parse_str(id).expect("Bad UUID");

        let fmt = "%Y-%m-%d";
        let date_obtained = NaiveDate::parse_from_str(date_obtained, fmt)
            .expect("Unable to parse date_obtained NaiveDate for LuckymonHistory.");

        LuckymonHistory {id, user_id, date_obtained, pokemon_id, shiny, pokemon_name}
    }

    pub fn to_hist(hist_map: HashMap<String, Value>) -> Self {
        LuckymonHistory::new(
            hist_map.get("id").unwrap().as_str().unwrap(),
            hist_map.get("user_id").unwrap().as_i64().unwrap(),
            hist_map.get("date_obtained").unwrap().as_str().unwrap(),
            hist_map.get("pokemon_id").unwrap().as_i64().unwrap(),
            hist_map.get("shiny").unwrap().as_bool().unwrap(),
            hist_map.get("pokemon_name").unwrap().as_str().unwrap().to_string()
        )
    }
}

#[command]
#[description = "Retrieves Luckymon History for a User."]
async fn luckydex(ctx: &Context, msg: &Message) -> CommandResult {
    println!("Got luckydex command..");
    let resp = reqwest::get(format!("http://localhost:8000/api/v1/luckymon-history/user-id/{}", i64::from(msg.author.id)))
        .await?
        .json::<Vec<HashMap<String, Value>>>()
        .await?;
    let mut hists: Vec<LuckymonHistory> = Vec::new();
    for hist_map in resp {
        hists.push(LuckymonHistory::to_hist(hist_map));
    }

    let items_per_page = 10;
    let total_pages = (hists.len() as f64 / items_per_page as f64).ceil() as usize;
    let mut current_page = 0;

    let mut message = create_embed_page(ctx, msg, &hists, items_per_page, current_page).await?;

    while let Some(interaction) = message
            .await_component_interaction(&ctx)
            .timeout(Duration::from_secs(120))
            .await
    {
        if interaction.user.id != msg.author.id {
            return Ok(())
        }

        let custom_id = &interaction.data.custom_id;
        if custom_id == "prev" && current_page > 0 {
            current_page -= 1;
        } else if custom_id == "next" && current_page < total_pages - 1 {
            current_page += 1;
        }

        message = update_embed_page(ctx, &mut message, &hists, items_per_page, current_page, &msg).await?;

        interaction
            .create_interaction_response(&ctx.http, |r| {
                r.kind(InteractionResponseType::UpdateMessage);
                r.interaction_response_data(|d| {
                    d.set_embed(message.embeds[0].clone().into());
                    d
                })
            })
            .await?;
    }

    println!("Finished processing luckydex command!");
    Ok(())
}

async fn create_embed_page(
    ctx: &Context,
    msg: &Message,
    data: &[LuckymonHistory],
    items_per_page: usize,
    current_page: usize,
) -> serenity::Result<Message> {
    let start_index = current_page * items_per_page;
    let end_index = usize::min(start_index + items_per_page, data.len());

    let current_data = &data[start_index..end_index];

    let mut pokemon_entry = Vec::new();

    for hist in current_data {
        let pokedex_number = hist.pokemon_id;
        let date = hist.date_obtained;

        let mut name = hist.pokemon_name.clone();
        if hist.shiny {
            name = format!("✨{}✨", hist.pokemon_name.clone())
        }

        if !hist.shiny {
            pokemon_entry.push(format!("`{:<10} {:<17} {:<22}`", pokedex_number, name, date));
        } else {
            pokemon_entry.push(format!("`{:<10} {:<14} {:<22}`", pokedex_number, name, date));
        }
    }
    let mut total_pages = (data.len() as f64 / items_per_page as f64).ceil() as usize;
    if total_pages == 0 {
        total_pages = 1;
    }

    let action_row = CreateActionRow::default()
        .create_button(|b| {
            b.style(ButtonStyle::Primary)
                .custom_id("prev")
                .disabled(current_page == 0)
                .label("Previous")
        })
        .create_button(|b| {
            b.style(ButtonStyle::Primary)
                .custom_id("next")
                .disabled(current_page == total_pages - 1)
                .label("Next")
        })
        .clone();

    let components = CreateComponents::default().add_action_row(action_row).clone();

    msg.channel_id
        .send_message(&ctx.http, |m| {
            m.embed(|e| {
                e.title("Luckydex")
                    .color(Colour::from_rgb(0, 255, 255))
                    .footer(|f| {
                        f.text(
                            format!(
                                "{}: Page {} of {}",
                                &msg.author.name,
                                current_page + 1,
                                total_pages
                            )
                        );
                        f.icon_url(&msg.author.avatar_url().unwrap())
                    });

                e.field(
                    "Pokédex #   Pokémon's Name   Obtained (YYYY-MM-DD)",
                    pokemon_entry.iter().map(|s| s.to_string()).collect::<Vec<String>>().join("\n"),
                    true,
                );

                e.timestamp(Timestamp::now());

                e
            });
            m.set_components(components.clone())
        })
        .await
}

async fn update_embed_page(
    ctx: &Context,
    msg: &mut Message,
    data: &[LuckymonHistory],
    items_per_page: usize,
    current_page: usize,
    original_owner: &Message
) -> serenity::Result<Message> {
    let start_index = current_page * items_per_page;
    let end_index = usize::min(start_index + items_per_page, data.len());

    let current_data = &data[start_index..end_index];

    let mut pokemon_entry = Vec::new();

    for hist in current_data {
        let pokedex_number = hist.pokemon_id;
        let date = hist.date_obtained;

        let mut name = hist.pokemon_name.clone();
        if hist.shiny {
            name = format!("✨{}✨", hist.pokemon_name.clone())
        }

        if !hist.shiny {
            pokemon_entry.push(format!("`{:<10} {:<17} {:<22}`", pokedex_number, name, date));
        } else {
            pokemon_entry.push(format!("`{:<10} {:<14} {:<22}`", pokedex_number, name, date));
        }
    }

    let mut total_pages = (data.len() as f64 / items_per_page as f64).ceil() as usize;
    if total_pages == 0 {
        total_pages = 1;
    }

    let author_name = &original_owner.author.name.clone();
    let avatar_url = &original_owner.author.avatar_url().unwrap().clone();

    let action_row = CreateActionRow::default()
        .create_button(|b| {
            b.style(ButtonStyle::Primary)
                .custom_id("prev")
                .disabled(current_page == 0)
                .label("Previous")
        })
        .create_button(|b| {
            b.style(ButtonStyle::Primary)
                .custom_id("next")
                .disabled(current_page == total_pages - 1)
                .label("Next")
        })
        .clone();

    let components = CreateComponents::default().add_action_row(action_row).clone();

    msg.channel_id
        .edit_message(&ctx.http, msg.id, |m| {
        m.embed(|e| {
            e.title("Luckydex")
                .color(Colour::from_rgb(0, 255, 255))
                .footer(|f| {
                    f.text(
                        format!(
                            "{}: Page {} of {}",
                            author_name,
                            current_page + 1,
                            total_pages
                        )
                    );
                    f.icon_url(avatar_url)
                });

            e.field(
                "Pokédex #   Pokémon's Name   Obtained (YYYY-MM-DD)",
                pokemon_entry.iter().map(|s| s.to_string()).collect::<Vec<String>>().join("\n"),
                true,
            );

            e.timestamp(Timestamp::now());

            e
        });
        m.set_components(components.clone())
    })
    .await
}

