use std::collections::HashMap;
use std::time::Duration;

use serenity::builder::{CreateActionRow, CreateComponents};
use serenity::framework::standard::macros::command;
use serenity::framework::standard::CommandResult;
use serenity::model::application::component::ButtonStyle;
use serenity::model::application::interaction::InteractionResponseType;
use serenity::model::channel::Message;
use serenity::model::Timestamp;
use serenity::prelude::*;
use serenity::utils::Colour;

use chrono::NaiveDate;
use image::{imageops, ImageBuffer, Rgba};
use imageproc::drawing::draw_text_mut;
use imageproc::rect::Rect;
use rand::Rng;
use rusttype::{point, Font, PositionedGlyph, Scale};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug)]
pub struct LuckymonHistory {
    pub id: Uuid,
    pub user_id: i64,
    pub date_obtained: NaiveDate,
    pub pokemon_id: i64,
    pub shiny: bool,
    pub pokemon_name: String,
}

impl LuckymonHistory {
    pub fn new(
        id: &str,
        user_id: i64,
        date_obtained: &str,
        pokemon_id: i64,
        shiny: bool,
        pokemon_name: String,
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
        )
    }
}

#[command]
#[description = "Retrieves Luckymon History for a User."]
async fn luckydex(ctx: &Context, msg: &Message) -> CommandResult {
    println!("Got luckydex command..");
    create_page_image();
    let resp = reqwest::get(format!(
        "http://localhost:8000/api/v1/luckymon-history/user-id/{}",
        i64::from(msg.author.id)
    ))
    .await?
    .json::<Vec<HashMap<String, Value>>>()
    .await?;
    let mut hists: Vec<LuckymonHistory> = Vec::new();
    for hist_map in resp {
        hists.push(LuckymonHistory::to_hist(hist_map));
    }

    let items_per_page = 9;
    let total_pages = (hists.len() as f64 / items_per_page as f64).ceil() as usize;
    let mut current_page = 0;

    let mut message = create_embed_page(ctx, msg, &hists, items_per_page, current_page).await?;

    while let Some(interaction) = message
        .await_component_interaction(&ctx)
        .timeout(Duration::from_secs(120))
        .await
    {
        if interaction.user.id != msg.author.id {
            interaction
                .create_interaction_response(&ctx.http, |r| {
                    r.kind(InteractionResponseType::UpdateMessage);
                    r.interaction_response_data(|d| {
                        d.set_embed(message.embeds[0].clone().into());
                        d
                    })
                })
                .await?;

            continue;
        }

        let custom_id = &interaction.data.custom_id;
        if custom_id == "prev" && current_page > 0 {
            current_page -= 1;
        } else if custom_id == "next" && current_page < total_pages - 1 {
            current_page += 1;
        }

        message = update_embed_page(
            ctx,
            &mut message,
            &hists,
            items_per_page,
            current_page,
            &msg,
        )
        .await?;

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
            pokemon_entry.push(format!(
                "`{:<10} {:<17} {:<22}`",
                pokedex_number, name, date
            ));
        } else {
            pokemon_entry.push(format!(
                "`{:<10} {:<14} {:<22}`",
                pokedex_number, name, date
            ));
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

    let components = CreateComponents::default()
        .add_action_row(action_row)
        .clone();

    msg.channel_id
        .send_message(&ctx.http, |m| {
            m.embed(|e| {
                e.title("Luckydex")
                    .color(Colour::from_rgb(0, 255, 255))
                    .footer(|f| {
                        f.text(format!(
                            "{}: Page {} of {}",
                            &msg.author.name,
                            current_page + 1,
                            total_pages
                        ));
                        f.icon_url(&msg.author.avatar_url().unwrap())
                    });

                e.field(
                    "Pokédex #   Pokémon's Name   Obtained (YYYY-MM-DD)",
                    pokemon_entry
                        .iter()
                        .map(|s| s.to_string())
                        .collect::<Vec<String>>()
                        .join("\n"),
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
    original_owner: &Message,
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
            pokemon_entry.push(format!(
                "`{:<10} {:<17} {:<22}`",
                pokedex_number, name, date
            ));
        } else {
            pokemon_entry.push(format!(
                "`{:<10} {:<14} {:<22}`",
                pokedex_number, name, date
            ));
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

    let components = CreateComponents::default()
        .add_action_row(action_row)
        .clone();

    msg.channel_id
        .edit_message(&ctx.http, msg.id, |m| {
            m.embed(|e| {
                e.title("Luckydex")
                    .color(Colour::from_rgb(0, 255, 255))
                    .footer(|f| {
                        f.text(format!(
                            "{}: Page {} of {}",
                            author_name,
                            current_page + 1,
                            total_pages
                        ));
                        f.icon_url(avatar_url)
                    });

                e.field(
                    "Pokédex #   Pokémon's Name   Obtained (YYYY-MM-DD)",
                    pokemon_entry
                        .iter()
                        .map(|s| s.to_string())
                        .collect::<Vec<String>>()
                        .join("\n"),
                    true,
                );

                e.timestamp(Timestamp::now());

                e
            });
            m.set_components(components.clone())
        })
        .await
}

fn create_page_image() -> ImageBuffer<Rgba<u8>, Vec<u8>> {
    let bg_root_path = "./resources/luckydex/";
    let sprite_root_path = "./resources/sprites/";
    let bg_dimensions = 500; // all backgrounds are 500x500
    let sprite_dimensions = 96; // all sprites are 96x96
    let grid_dimensions = 3; // 3 rows and 3 columns per page
    let background_filename = format!("bg{}.png", rand::thread_rng().gen_range(1..=20)); // 1 - 20

    let y_spacing_buffer = sprite_dimensions - 10;
    let x_spacing_buffer = ((bg_dimensions / grid_dimensions) / 2) - 23;

    // Calculate spacing
    let sprite_spacing = (bg_dimensions - (grid_dimensions * sprite_dimensions)) / grid_dimensions;

    let mut img = ImageBuffer::new(bg_dimensions, bg_dimensions);
    let background = image::open(format!("{}{}", bg_root_path, background_filename))
        .unwrap()
        .to_rgba8();

    imageops::overlay(&mut img, &background, 0, 0);

    // Draw horizontal grid lines
    for i in 1..grid_dimensions {
        let y = (bg_dimensions / grid_dimensions) * i;
        for x in 0..bg_dimensions {
            img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
        }
    }

    // Draw vertical grid lines
    for i in 1..grid_dimensions {
        let x = (bg_dimensions / grid_dimensions) * i;
        for y in 0..bg_dimensions {
            img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
        }
    }

    let font = get_font();

    let font_height: f32 = 14.0; // Set the font size
    let font_scale = Scale {
        x: font_height,
        y: font_height,
    };

    for row in 0..grid_dimensions {
        for col in 0..grid_dimensions {
            let random_pokemon = rand::thread_rng().gen_range(1..=1010);
            let pokemon = image::open(format!("{}{}.png", sprite_root_path, random_pokemon))
                .unwrap()
                .to_rgba8();

            let x: i64 = (((col * (sprite_dimensions + sprite_spacing)) + sprite_dimensions)
                - x_spacing_buffer)
                .into();
            let y: i64 = (((row * (sprite_dimensions + sprite_spacing)) + sprite_dimensions)
                - y_spacing_buffer)
                .into();
            imageops::overlay(&mut img, &pokemon, x, y);

            let texts = vec!["Pokemon Name", "1234", "09/01/23"];
            let mut text_spacing = 8;
            for text in texts {
                let text_width = text_width(&font, font_scale, text);
                let text_x = x + ((sprite_dimensions as f32 - text_width) / 2.0).round() as i64;
                let text_y = y + sprite_dimensions as i64 + text_spacing as i64;
                draw_text_mut(
                    &mut img,
                    Rgba([0, 0, 0, 255]),
                    text_x.try_into().unwrap(),
                    text_y.try_into().unwrap(),
                    font_scale,
                    &font,
                    text,
                );
                text_spacing = text_spacing + 15;
            }
        }
    }

    img.save("test_image.png").unwrap();

    return img;
}

fn get_font<'a>() -> Font<'a> {
    let font_data: &[u8] = include_bytes!("../../resources/fonts/DejaVuSans.ttf");
    return Font::try_from_bytes(font_data).unwrap();
}

// Ultimately used to center the text that is written over the generated image
fn text_width(font: &Font, scale: Scale, text: &str) -> f32 {
    let v_metrics = font.v_metrics(scale);
    let glyphs: Vec<PositionedGlyph<'_>> = font
        .layout(text, scale, point(0.0, v_metrics.ascent))
        .collect();
    let width = glyphs
        .iter()
        .rev()
        .map(|g| g.position().x as f32 + g.unpositioned().h_metrics().advance_width)
        .next()
        .unwrap_or(0.0);
    width
}
