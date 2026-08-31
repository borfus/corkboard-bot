use std::collections::HashMap;
use std::time::Duration;

use serenity::builder::{CreateActionRow, CreateEmbed};
use serenity::model::application::component::ButtonStyle;
use serenity::model::application::interaction::InteractionResponseType;
use serenity::model::Timestamp;
use serenity::prelude::*;
use serenity::utils::Colour;

use super::image_host;
use crate::slash::Invocation;

use chrono::NaiveDate;
use image::{imageops, ImageBuffer, Rgba};
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

/// One page is the full 10x10 grid drawn by `create_page_image`.
const ITEMS_PER_PAGE: usize = 100;
const PAGE_TIMEOUT_SECS: u64 = 120;

/// `/luckydex` -- your collection, ephemeral.
///
/// Private for the same reason `/luckyteam` is: a dex is only interesting to
/// the person who owns it, and posting one publicly every time somebody wants a
/// look turns a busy channel into a wall of bot images.
///
/// Because the reply is ephemeral, the "only the caller may page" check the
/// prefix version needed is gone -- nobody else can see the buttons to click.
pub async fn slash_luckydex(inv: &Invocation<'_>) -> serenity::Result<()> {
    inv.defer(true).await?;

    let ctx = inv.ctx;
    let user_id = i64::from(inv.user_id());

    let resp = match reqwest::get(format!(
        "http://localhost:8000/api/v1/luckymon-history/user-id/{}",
        user_id
    ))
    .await
    {
        Ok(r) => match r.json::<Vec<HashMap<String, Value>>>().await {
            Ok(list) => list,
            Err(_) => return inv.fail("Could not read your luckydex.").await,
        },
        Err(_) => return inv.fail("Could not reach the server.").await,
    };

    let hists: Vec<LuckymonHistory> = resp
        .into_iter()
        .map(LuckymonHistory::to_hist)
        .filter(|h| !h.traded)
        .collect();

    let total_pages = ((hists.len() as f64 / ITEMS_PER_PAGE as f64).ceil() as usize).max(1);
    let mut current_page = 0usize;

    let (author_name, avatar_url) = inv.author_identity().await;

    // Held across page turns so the loading state can show the picture already
    // on screen rather than a blank frame.
    let mut image_url = render_page(ctx, &hists, current_page).await;

    inv.command
        .edit_original_interaction_response(&ctx.http, |r| {
            r.set_embed(page_embed(
                &image_url,
                &author_name,
                &avatar_url,
                current_page,
                total_pages,
                false,
            ));
            r.components(|c| c.add_action_row(page_buttons(current_page, total_pages, true)))
        })
        .await?;

    let dex_msg = inv.command.get_interaction_response(&ctx.http).await?;

    while let Some(interaction) = dex_msg
        .await_component_interaction(ctx)
        .timeout(Duration::from_secs(PAGE_TIMEOUT_SECS))
        .await
    {
        let target = match interaction.data.custom_id.as_str() {
            "prev" if current_page > 0 => current_page - 1,
            "next" if current_page + 1 < total_pages => current_page + 1,
            _ => current_page,
        };

        // Answer the click with a visible change rather than a silent
        // acknowledgement: greyed-out buttons and a "Loading page N" footer, so
        // the wait reads as progress instead of a broken button. This also
        // rules out a second click landing while the first is still drawing.
        interaction
            .create_interaction_response(&ctx.http, |r| {
                r.kind(InteractionResponseType::UpdateMessage)
                    .interaction_response_data(|d| {
                        d.set_embed(page_embed(
                            &image_url,
                            &author_name,
                            &avatar_url,
                            target,
                            total_pages,
                            true,
                        ))
                        .components(|c| {
                            c.add_action_row(page_buttons(target, total_pages, false))
                        })
                    })
            })
            .await?;

        if target != current_page {
            current_page = target;
            image_url = render_page(ctx, &hists, current_page).await;
        }

        let _ = interaction
            .edit_original_interaction_response(&ctx.http, |r| {
                r.set_embed(page_embed(
                    &image_url,
                    &author_name,
                    &avatar_url,
                    current_page,
                    total_pages,
                    false,
                ));
                r.components(|c| {
                    c.add_action_row(page_buttons(current_page, total_pages, true))
                })
            })
            .await;
    }

    // The collector has stopped listening. Strip the buttons so a later click
    // does not sit unanswered.
    let _ = inv
        .command
        .edit_original_interaction_response(&ctx.http, |r| r.components(|c| c))
        .await;

    Ok(())
}

/// The pager controls.
///
/// `enabled` goes false while a page is being drawn: a 10x10 page is around a
/// megabyte to composite and upload, which is long enough that an unchanged
/// message reads as a dead button. Greying the controls out both shows that
/// something is happening and makes a second click impossible.
fn page_buttons(current_page: usize, total_pages: usize, enabled: bool) -> CreateActionRow {
    CreateActionRow::default()
        .create_button(|b| {
            b.style(ButtonStyle::Primary)
                .custom_id("prev")
                .disabled(!enabled || current_page == 0)
                .label("Previous")
        })
        .create_button(|b| {
            b.style(ButtonStyle::Primary)
                .custom_id("next")
                .disabled(!enabled || current_page + 1 >= total_pages)
                .label("Next")
        })
        .clone()
}

/// Composites a page and parks it in the staging channel, returning its URL.
async fn render_page(ctx: &Context, data: &[LuckymonHistory], page: usize) -> String {
    let start_index = page * ITEMS_PER_PAGE;
    let end_index = usize::min(start_index + ITEMS_PER_PAGE, data.len());

    let luckydex_page = create_page_image(&data[start_index..end_index]);
    image_host::host(ctx, &luckydex_page).await
}

/// Builds the embed around an already-hosted image.
///
/// Kept separate from rendering so the loading state can reuse the picture
/// that is already on screen -- drawing a second one just to say "please wait"
/// would cost exactly as much as the page being waited for.
fn page_embed(
    image_url: &str,
    author_name: &str,
    avatar_url: &Option<String>,
    page: usize,
    total_pages: usize,
    loading: bool,
) -> CreateEmbed {
    let footer = if loading {
        format!(
            "{}: Loading page {} of {}\u{2026}",
            author_name,
            page + 1,
            total_pages
        )
    } else {
        format!("{}: Page {} of {}", author_name, page + 1, total_pages)
    };

    let mut embed = CreateEmbed::default();
    embed
        .title("Luckydex")
        .color(Colour::from_rgb(0, 255, 255))
        .image(image_url)
        .timestamp(Timestamp::now())
        .footer(|f| {
            f.text(footer);
            if let Some(avatar_url) = avatar_url {
                f.icon_url(avatar_url);
            }
            f
        });
    embed
}

fn create_page_image(data: &[LuckymonHistory]) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
    let bg_root_path = "./resources/luckydex/";
    let sprite_root_path = "./resources/sprites/";
    // Canvas scales with the grid rather than the cells: a 96px sprite plus its
    // three lines of text needs about 165px of room, so ten columns needs 1650.
    // Shrinking cells to keep the old 825px canvas would leave no space for
    // either.
    let grid_dimensions = 10; // 10 rows and 10 columns per page
    let sprite_dimensions = 96; // all sprites are 96x96
    let bg_dimensions = 165 * grid_dimensions; // background dimensions x and y
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

                pokemon_name = format!("\u{2727}\u{02D6}\u{00B0} Shiny {} \u{00B0}\u{02D6}\u{2727}", pokemon_name);
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
                format!("Pok\u{00E9}dex #: {}", pokemon_data.pokemon_id),
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Renders a full page to disk for eyeballing. Ignored by default: it needs
    /// `resources/` present and produces a file rather than an assertion.
    #[test]
    #[ignore]
    fn preview_luckydex_page() {
        let data: Vec<LuckymonHistory> = (1..=ITEMS_PER_PAGE)
            .map(|i| LuckymonHistory {
                id: Uuid::new_v4(),
                user_id: 1,
                date_obtained: NaiveDate::from_ymd_opt(2026, 8, 31).unwrap(),
                pokemon_id: i as i64,
                shiny: i % 17 == 0,
                pokemon_name: format!("pokemon-{}", i),
                traded: false,
            })
            .collect();

        let img = create_page_image(&data);
        let expected = 165 * 10;
        assert_eq!(img.width(), expected);
        assert_eq!(img.height(), expected);

        let path = std::env::var("DEX_PREVIEW_PATH")
            .unwrap_or_else(|_| "luckydex_preview.png".to_string());
        img.save(&path).expect("could not write preview");
        println!("wrote {} ({}x{})", path, img.width(), img.height());
    }

    /// The grid only lines up while the canvas is an exact multiple of the cell
    /// size -- the sprite and text offsets are derived from integer division.
    #[test]
    fn canvas_divides_evenly_into_cells() {
        let grid: u32 = 10;
        let canvas: u32 = 165 * grid;
        let sprite: u32 = 96;

        assert_eq!(canvas % grid, 0, "cells must be a whole number of pixels");
        assert_eq!(
            (canvas - grid * sprite) % grid,
            0,
            "sprite spacing must divide evenly too"
        );

        // A cell has to hold the sprite plus three lines of text beneath it.
        let cell = canvas / grid;
        assert!(
            cell >= sprite + 45,
            "cell of {}px is too small for a {}px sprite and its labels",
            cell,
            sprite
        );
    }

    #[test]
    fn a_page_holds_the_whole_grid() {
        assert_eq!(ITEMS_PER_PAGE, 10 * 10);
    }
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
