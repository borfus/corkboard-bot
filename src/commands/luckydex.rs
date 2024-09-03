use std::collections::HashMap;
use std::io::Cursor;
use std::time::Duration;

use serenity::builder::{CreateActionRow, CreateComponents};
use serenity::framework::standard::macros::command;
use serenity::framework::standard::CommandResult;
use serenity::model::application::component::ButtonStyle;
use serenity::model::application::interaction::InteractionResponseType;
use serenity::model::channel::Message;
use serenity::model::prelude::AttachmentType;
use serenity::model::prelude::*;
use serenity::model::Timestamp;
use serenity::prelude::*;
use serenity::utils::Colour;

use chrono::NaiveDate;
use image::codecs::png::PngEncoder;
use image::{imageops, ImageBuffer, ImageEncoder, Rgba};
use imageproc::drawing::draw_text_mut;
use rand::Rng;
use rusttype::{point, Font, PositionedGlyph, Scale};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use super::luckymon;

#[derive(Serialize, Deserialize, Debug)]
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

#[command]
#[description = "Retrieves Luckymon History for a User."]
async fn luckydex(ctx: &Context, msg: &Message) -> CommandResult {
    println!("Got luckydex command..");
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

    let items_per_page = 25;
    let total_pages = (hists.len() as f64 / items_per_page as f64).ceil() as usize;
    let mut current_page = 0;

    let mut message = create_embed_page(ctx, msg, &hists, items_per_page, current_page).await?;

    while let Some(interaction) = message
        .await_component_interaction(&ctx)
        .timeout(Duration::from_secs(120))
        .await
    {
        interaction
            .create_interaction_response(&ctx.http, |r| {
                r.kind(InteractionResponseType::DeferredUpdateMessage)
            })
            .await?;

        if interaction.user.id != msg.author.id {
            interaction
                .edit_original_interaction_response(&ctx.http, |r| {
                    r.set_embed(message.embeds[0].clone().into())
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
            .edit_original_interaction_response(&ctx.http, |r| {
                r.set_embed(message.embeds[0].clone().into())
            })
            .await?;
    }

    println!("Finished processing luckydex command!");
    Ok(())
}

#[allow(deprecated)]
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
    let luckydex_page = create_page_image(current_data);

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

    let mut buffer: Vec<u8> = Vec::new();
    {
        let mut writer = Cursor::new(&mut buffer);
        let encoder = PngEncoder::new(&mut writer);
        encoder
            .write_image(
                &luckydex_page,
                luckydex_page.width(),
                luckydex_page.height(),
                image::ColorType::Rgba8,
            )
            .expect("Error encoding image");
    }

    let image_url = send_dummy_message(&ctx, &buffer).await;

    msg.channel_id
        .send_message(&ctx.http, |m| {
            m.embed(|e| {
                e.title("Luckydex")
                    .image("attachment://image.png")
                    .color(Colour::from_rgb(0, 255, 255))
                    .footer(|f| {
                        f.text(format!(
                            "{}: Page {} of {}",
                            &msg.author.name,
                            current_page + 1,
                            total_pages
                        ));
                        if let Some(avatar_url) = &msg.author.avatar_url() {
                            f.icon_url(avatar_url);
                        }
                        f
                    });
                e.image(image_url);
                e.timestamp(Timestamp::now());
                e
            });
            m.set_components(components.clone())
        })
        .await
}

#[allow(deprecated)]
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
    let luckydex_page = create_page_image(current_data);

    let mut total_pages = (data.len() as f64 / items_per_page as f64).ceil() as usize;
    if total_pages == 0 {
        total_pages = 1;
    }

    let author_name = &original_owner.author.name.clone();
    let avatar_url = &original_owner.author.avatar_url();

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

    let mut buffer: Vec<u8> = Vec::new();
    {
        let mut writer = Cursor::new(&mut buffer);
        let encoder = PngEncoder::new(&mut writer);
        encoder
            .encode(
                &luckydex_page,
                luckydex_page.width(),
                luckydex_page.height(),
                image::ColorType::Rgba8,
            )
            .expect("Error encoding image");
    }

    let image_url = send_dummy_message(&ctx, &buffer).await;

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
                        if let Some(avatar_url) = avatar_url {
                            f.icon_url(avatar_url);
                        }
                        f
                    });
                e.image(image_url);
                e.timestamp(Timestamp::now());
                e
            });
            m.set_components(components.clone())
        })
        .await
}

fn create_page_image(data: &[LuckymonHistory]) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
    let bg_root_path = "./resources/luckydex/";
    let sprite_root_path = "./resources/sprites/";
    let bg_dimensions = 825; // background dimensions x and y
    let sprite_dimensions = 96; // all sprites are 96x96
    let grid_dimensions = 5; // 5 rows and 5 columns per page
    let background_filename = format!("bg{}.png", rand::thread_rng().gen_range(1..=20)); // 1 - 20

    let y_spacing_buffer = sprite_dimensions - 10;
    let x_spacing_buffer = ((bg_dimensions / grid_dimensions) / 2) - 20;

    // Calculate spacing
    let sprite_spacing = (bg_dimensions - (grid_dimensions * sprite_dimensions)) / grid_dimensions;

    let mut img = ImageBuffer::new(bg_dimensions, bg_dimensions);
    let background_unfitted =
        image::open(format!("{}{}", bg_root_path, background_filename)).unwrap();
    let background = background_unfitted
        .resize(
            bg_dimensions,
            bg_dimensions,
            imageops::FilterType::CatmullRom,
        )
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

    let pokemon_count = data.len();
    for row in 0..grid_dimensions {
        for col in 0..grid_dimensions {
            let current_pokemon = col + (row * grid_dimensions);
            if current_pokemon >= pokemon_count.try_into().unwrap() {
                return img;
            }

            let pokemon_data: &LuckymonHistory = &data[current_pokemon as usize];
            let mut pokemon_sprite = image::open(format!(
                "{}{}.png",
                sprite_root_path, &pokemon_data.pokemon_id
            ))
            .unwrap()
            .to_rgba8();

            let mut pokemon_name = luckymon::format_for_display(pokemon_data.pokemon_name.as_str());

            if pokemon_data.shiny {
                pokemon_sprite = image::open(format!(
                    "{}{}_shiny.png",
                    sprite_root_path, &pokemon_data.pokemon_id
                ))
                .unwrap()
                .to_rgba8();

                pokemon_name = format!("✧˖° Shiny {} °˖✧", pokemon_name);
            } else {
            }

            let x: i64 = (((col * (sprite_dimensions + sprite_spacing)) + sprite_dimensions)
                - x_spacing_buffer)
                .into();
            let y: i64 = (((row * (sprite_dimensions + sprite_spacing)) + sprite_dimensions)
                - y_spacing_buffer)
                .into();
            imageops::overlay(&mut img, &pokemon_sprite, x, y);

            let texts = vec![
                pokemon_name,
                format!("Pokédex #: {}", pokemon_data.pokemon_id),
                pokemon_data.date_obtained.to_string(),
            ];
            let mut text_spacing = 8;
            for text in texts {
                let text_width = text_width(&font, font_scale, text.as_str());
                let text_x = x + ((sprite_dimensions as f32 - text_width) / 2.0).round() as i64;
                let text_y = y + sprite_dimensions as i64 + text_spacing as i64;
                draw_text_mut(
                    &mut img,
                    Rgba([0, 0, 0, 255]),
                    text_x.try_into().unwrap(),
                    text_y.try_into().unwrap(),
                    font_scale,
                    &font,
                    text.as_str(),
                );
                text_spacing = text_spacing + 15;
            }
        }
    }

    return img;
}

fn get_font<'a>() -> Font<'a> {
    let font_data: &[u8] = include_bytes!("../../resources/fonts/DejaVuSans.ttf");
    return Font::try_from_bytes(font_data).unwrap();
}

// Ultimately used to center the text that is written over the generated page image
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

async fn send_dummy_message(ctx: &Context, buffer: &Vec<u8>) -> String {
    // Send dummy message to specific Discord server & channel to upload the image to grab the url
    // Note: If you want to host this bot by yourself, you need to make a server with a dedicated channel
    // that this bot can post the generated images in to properly update the embedded message with a new image
    let channel_id: ChannelId = ChannelId(1155366534617763931);
    let guild_id: GuildId = GuildId(423944755118866444);

    let mut image_url = String::new();

    // Check if the channel is in the server
    if let Ok(channel) = channel_id.to_channel(&ctx).await {
        if let Some(guild_channel) = channel.guild() {
            if guild_channel.guild_id == guild_id {
                let files = vec![AttachmentType::Bytes {
                    data: buffer.into(),
                    filename: "image.png".to_string(),
                }];
                if let Ok(sent_message) = channel_id
                    .send_files(&ctx.http, files, |m| m.content(""))
                    .await
                {
                    // Extract the URL of the uploaded image
                    if let Some(attachment) = sent_message.attachments.first() {
                        image_url = attachment.url.clone();
                    }
                }
            }
        }
    }

    image_url
}
