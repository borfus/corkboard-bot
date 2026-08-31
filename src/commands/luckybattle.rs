//! `/luckybattle` -- 3v3 auto-resolved battles, PvP and PvE.
//!
//! The server decides everything. This command collects intent, asks for the
//! battle, and plays the returned log back by editing one embed.

use std::time::Duration;

use image::{imageops, ImageBuffer, Rgba};
use rand::Rng;
use imageproc::drawing::draw_line_segment_mut;
use serenity::builder::{CreateActionRow, CreateButton};
use serenity::model::id::{ChannelId, GuildId, UserId};
use serenity::model::prelude::component::ButtonStyle;
use serenity::model::prelude::InteractionResponseType;
use serenity::prelude::*;

use super::battle_api::{self, BattleLog, BattleOutcome, BattleRequest, MonSnapshot};
use super::image_host;
use super::luckymon::format_for_display;
use crate::slash::{member_display_name as display_name, require_guild, Invocation};

/// How long a playback frame stays on screen.
const FRAME_DELAY_MS: u64 = 1500;

/// Upper bound on frames, so a fifty-turn stall does not become a fifty-edit
/// rate-limit problem. Longer battles simply show more events per frame.
const MAX_FRAMES: usize = 12;

/// Log lines visible in a single frame.
const LINES_PER_FRAME: usize = 3;

/// How long a PvP challenge or a waiting PvE trainer holds. When it lapses the
/// buttons come off, because a button with nothing listening is worse than no
/// button -- Discord answers the click with "the application didn't respond in
/// time".
const CHALLENGE_TIMEOUT_SECS: u64 = 300;

/// `resources/luckydex/bg1.png` through `bg20.png`.
const BACKGROUND_COUNT: u32 = 20;

/// Ten-segment HP bar. A pokemon that is alive never shows an empty bar --
/// seeing a full-looking bar on something at 2% would read as a bug.
fn hp_bar(pct: i64) -> String {
    let pct = pct.clamp(0, 100);
    let mut filled = ((pct as f64 / 100.0) * 10.0).round() as usize;
    if pct > 0 && filled == 0 {
        filled = 1;
    }
    if pct == 0 {
        filled = 0;
    }
    format!("{}{}", "\u{2588}".repeat(filled), "\u{2591}".repeat(10 - filled))
}

fn label(mon: &MonSnapshot) -> String {
    let name = format_for_display(&mon.name);
    if mon.shiny {
        format!("\u{2728}{}", name)
    } else {
        name
    }
}

/// Renders one side's roster as bars. `hp` is that side's slice of the snapshot.
fn side_block(team: &[MonSnapshot], hp: &[i64]) -> String {
    team.iter()
        .enumerate()
        .map(|(i, mon)| {
            let pct = hp.get(i).copied().unwrap_or(0);
            let name = label(mon);
            let name = if name.chars().count() > 12 {
                name.chars().take(12).collect::<String>()
            } else {
                name
            };
            format!("`{:<12} {} {:>3}%`", name, hp_bar(pct), pct)
        })
        .collect::<Vec<String>>()
        .join("\n")
}

/// Splits the log into at most `MAX_FRAMES` frames.
fn frame_indices(total: usize) -> Vec<usize> {
    if total == 0 {
        return Vec::new();
    }
    if total <= MAX_FRAMES {
        return (0..total).collect();
    }

    let step = (total as f64) / (MAX_FRAMES as f64);
    let mut out: Vec<usize> = (0..MAX_FRAMES)
        .map(|i| (((i + 1) as f64 * step).round() as usize).saturating_sub(1))
        .collect();
    // Always land on the final event so the last frame shows the knockout.
    if let Some(last) = out.last_mut() {
        *last = total - 1;
    }
    out.dedup();
    out
}

fn recent_lines(log: &BattleLog, upto: usize) -> String {
    let start = (upto + 1).saturating_sub(LINES_PER_FRAME);
    log.turns[start..=upto]
        .iter()
        .map(|t| format!("\u{203A} {}", t.text))
        .collect::<Vec<String>>()
        .join("\n")
}

/// Picks a background for each side, guaranteed to differ so the two teams read
/// as standing on their own ground.
///
/// The second draw is taken from a range one smaller and then shifted past the
/// first pick, which keeps every remaining background equally likely -- a retry
/// loop would work too but has no bound on how long it runs.
fn pick_two_backgrounds<R: Rng>(rng: &mut R) -> (u32, u32) {
    let top = rng.gen_range(1..=BACKGROUND_COUNT);
    let mut bottom = rng.gen_range(1..BACKGROUND_COUNT);
    if bottom >= top {
        bottom += 1;
    }
    (top, bottom)
}

/// Paints one background into a horizontal band of the canvas.
///
/// The source images are 500x500, so each is cropped to the band rather than
/// overlaid whole -- an uncropped overlay would run past its half and be
/// painted over by the next one.
fn draw_background(
    img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    root: &str,
    index: u32,
    y: i64,
    width: u32,
    height: u32,
) {
    if let Ok(bg) = image::open(format!("{}bg{}.png", root, index)) {
        let bg = bg.to_rgba8();
        let w = width.min(bg.width());
        let h = height.min(bg.height());
        let band = imageops::crop_imm(&bg, 0, 0, w, h).to_image();
        imageops::overlay(img, &band, 0, y);
    }
}

/// Drains the colour out of a sprite in place.
///
/// Written by hand rather than via `imageops::grayscale`, which returns a
/// `Luma` buffer and drops the alpha channel -- a sprite that lost its
/// transparency would be a grey rectangle rather than a grey pokemon.
fn desaturate(img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>) {
    for pixel in img.pixels_mut() {
        let [r, g, b, a] = pixel.0;
        let luma =
            (0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32).round() as u8;
        pixel.0 = [luma, luma, luma, a];
    }
}

/// Paints a red X across one sprite slot.
fn draw_cross(img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, x: i64, y: i64, size: i64) {
    let red = Rgba([214u8, 48, 49, 255]);

    let inset = size as f32 * 0.16;
    let left = x as f32 + inset;
    let right = x as f32 + size as f32 - inset;
    let top = y as f32 + inset;
    let bottom = y as f32 + size as f32 - inset;

    // A single-pixel line all but disappears against a 96px sprite, so lay a
    // few alongside each other to get a stroke with some weight to it.
    for offset in -2..=2 {
        let o = offset as f32;
        draw_line_segment_mut(img, (left + o, top), (right + o, bottom), red);
        draw_line_segment_mut(img, (right + o, top), (left + o, bottom), red);
    }
}

/// The battle picture: two rows of three sprites, each row on its own
/// background.
///
/// Holds its backgrounds rather than choosing them per render, because the
/// image is redrawn mid-battle as pokemon faint and the ground underfoot
/// changing between frames would look like a different fight.
struct BattleScene {
    top_bg: u32,
    bottom_bg: u32,
}

impl BattleScene {
    fn new() -> Self {
        let (top_bg, bottom_bg) = pick_two_backgrounds(&mut rand::thread_rng());
        BattleScene { top_bg, bottom_bg }
    }

    fn render(
        &self,
        team_a: &[MonSnapshot],
        team_b: &[MonSnapshot],
        fainted_a: &[bool],
        fainted_b: &[bool],
    ) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
        let bg_root = "./resources/luckydex/";
        let sprite_root = "./resources/sprites/";
        let width: u32 = 400;
        let bg_height: u32 = 150;
        let height: u32 = bg_height * 2;
        let sprite_dim: i64 = 96;

        let mut img = ImageBuffer::new(width, height);

        draw_background(&mut img, bg_root, self.top_bg, 0, width, bg_height);
        draw_background(
            &mut img,
            bg_root,
            self.bottom_bg,
            bg_height as i64,
            width,
            bg_height,
        );

        let mut draw_row = |team: &[MonSnapshot], fainted: &[bool], y: i64| {
            for (i, mon) in team.iter().take(3).enumerate() {
                let file = if mon.shiny {
                    format!("{}{}_shiny.png", sprite_root, mon.pokemon_id)
                } else {
                    format!("{}{}.png", sprite_root, mon.pokemon_id)
                };

                // A missing shiny sprite falls back to the regular one rather
                // than leaving a hole in the line-up.
                let sprite = image::open(&file)
                    .or_else(|_| image::open(format!("{}{}.png", sprite_root, mon.pokemon_id)));

                if let Ok(sprite) = sprite {
                    let mut sprite = sprite.to_rgba8();
                    let down = fainted.get(i).copied().unwrap_or(false);

                    if down {
                        desaturate(&mut sprite);
                    }

                    let slot_width = width as i64 / 3;
                    let x = (i as i64 * slot_width) + (slot_width - sprite_dim) / 2;
                    imageops::overlay(&mut img, &sprite, x, y);

                    // Drawn onto the canvas rather than the sprite so the arms
                    // of the X are not clipped by the sprite's own bounds.
                    if down {
                        draw_cross(&mut img, x, y, sprite_dim);
                    }
                }
            }
        };

        let y_offset = (bg_height as i64 - sprite_dim) / 2;
        draw_row(team_a, fainted_a, y_offset);
        draw_row(team_b, fainted_b, bg_height as i64 + y_offset);

        img
    }
}

/// Which slots are down, read off a health snapshot.
///
/// The server guarantees 0% means fainted and nothing else, so this needs no
/// tolerance: a pokemon hanging on at a fraction of a percent still reports 1.
fn fainted_from(hp: &[i64]) -> Vec<bool> {
    hp.iter().map(|pct| *pct == 0).collect()
}

/// Plays the battle back and leaves the final state on screen.
///
/// The battle runs in the channel rather than as the interaction reply: it is a
/// spectacle, and everyone should see it. The image is hosted rather than
/// attached so that the dozen edits that follow all point at a stable URL.
async fn play_back(
    ctx: &Context,
    channel_id: ChannelId,
    outcome: &BattleOutcome,
    challenger_name: &str,
    opponent_name: &str,
) -> serenity::Result<()> {
    let log = &outcome.log;

    // The picture is redrawn as pokemon go down, so the scene keeps its
    // backgrounds and only the sprites change.
    let scene = BattleScene::new();
    let mut fainted_a = vec![false; log.team_a.len()];
    let mut fainted_b = vec![false; log.team_b.len()];

    let mut image_url = image_host::host(
        ctx,
        &scene.render(&log.team_a, &log.team_b, &fainted_a, &fainted_b),
    )
    .await;

    let header = format!(
        "{}  **{}** \u{2014} {}",
        log.battlefield_emoji, log.battlefield_name, log.battlefield_summary
    );

    // Opening frame: everyone at full health, nothing has happened yet.
    let full_a: Vec<i64> = log.team_a.iter().map(|_| 100).collect();
    let full_b: Vec<i64> = log.team_b.iter().map(|_| 100).collect();

    let mut battle_msg = channel_id
        .send_message(&ctx.http, |m| {
            m.embed(|e| {
                e.title(format!("{} vs {}", challenger_name, opponent_name))
                    .description(format!(
                        "{}\n\n**{}**\n{}\n\n**{}**\n{}",
                        header,
                        challenger_name,
                        side_block(&log.team_a, &full_a),
                        opponent_name,
                        side_block(&log.team_b, &full_b)
                    ))
                    .image(&image_url)
                    .footer(|f| f.text("Battle starting..."))
            })
        })
        .await?;

    let frames = frame_indices(log.turns.len());

    // The closing edit draws the last frame itself, so the loop stops one short
    // of it. Drawing it twice put two edits back to back with no gap between
    // them -- the exact shape that trips Discord's edit rate limit, and if the
    // second one lost that race "Battle in progress..." was what stayed on
    // screen.
    let playback = frames.len().saturating_sub(1);

    for idx in frames.iter().take(playback) {
        tokio::time::sleep(Duration::from_millis(FRAME_DELAY_MS)).await;

        let hp = &log.turns[*idx].hp_after;
        refresh_scene(
            ctx,
            &scene,
            log,
            hp,
            &mut fainted_a,
            &mut fainted_b,
            &mut image_url,
        )
        .await;

        let body = format!(
            "{}\n\n**{}**\n{}\n\n**{}**\n{}\n\n{}",
            header,
            challenger_name,
            side_block(&log.team_a, &hp.a),
            opponent_name,
            side_block(&log.team_b, &hp.b),
            recent_lines(log, *idx)
        );

        let _ = battle_msg
            .edit(&ctx.http, |m| {
                m.embed(|e| {
                    e.title(format!("{} vs {}", challenger_name, opponent_name))
                        .description(body)
                        .image(&image_url)
                        .footer(|f| f.text("Battle in progress..."))
                })
            })
            .await;
    }

    // Final frame: paced like the others, and the only place the closing state
    // is drawn.
    let winner_name = if log.winner == "a" {
        challenger_name
    } else {
        opponent_name
    };

    let (final_hp, closing_lines) = match frames.last() {
        Some(idx) => {
            tokio::time::sleep(Duration::from_millis(FRAME_DELAY_MS)).await;

            let hp = log.turns[*idx].hp_after.clone();
            refresh_scene(
                ctx,
                &scene,
                log,
                &hp,
                &mut fainted_a,
                &mut fainted_b,
                &mut image_url,
            )
            .await;

            // Carrying the last few log lines through means the closing message
            // still shows the knockout that decided it.
            (hp, format!("\n\n{}", recent_lines(log, *idx)))
        }
        None => (
            battle_api::HpSnapshot {
                a: full_a.clone(),
                b: full_b.clone(),
            },
            String::new(),
        ),
    };

    let mut footer = if log.decided_on_hp {
        "Turn limit reached \u{2014} decided on remaining HP.".to_string()
    } else {
        String::new()
    };
    if !outcome.counted {
        if !footer.is_empty() {
            footer.push(' ');
        }
        footer.push_str("Not counted toward the leaderboard.");
    }
    if footer.is_empty() {
        footer = "Use /luckyboard to see the standings.".to_string();
    }

    // Not `let _ =`: this is the edit whose failure a spectator would actually
    // notice, because the message would be left reading "Battle in progress..."
    // forever. Worth a line in the log rather than silence.
    if let Err(why) = battle_msg
        .edit(&ctx.http, |m| {
            m.embed(|e| {
                e.title(format!("\u{1F3C6} {} wins!", winner_name))
                    .description(format!(
                        "{}\n\n**{}**\n{}\n\n**{}**\n{}{}",
                        header,
                        challenger_name,
                        side_block(&log.team_a, &final_hp.a),
                        opponent_name,
                        side_block(&log.team_b, &final_hp.b),
                        closing_lines
                    ))
                    .image(&image_url)
                    .footer(|f| f.text(footer))
            })
        })
        .await
    {
        println!("Failed to post the battle result: {:?}", why);
    }

    // No components on the result message, so there is nothing here that can
    // go stale and start swallowing clicks.
    Ok(())
}

/// Redraws and re-hosts the battle picture, but only when the set of fainted
/// slots has actually changed.
///
/// Every redraw is an upload, and a 3v3 can only produce six knockouts --
/// repainting an unchanged picture on every frame would be wasted round trips.
async fn refresh_scene(
    ctx: &Context,
    scene: &BattleScene,
    log: &BattleLog,
    hp: &battle_api::HpSnapshot,
    fainted_a: &mut Vec<bool>,
    fainted_b: &mut Vec<bool>,
    image_url: &mut String,
) {
    let next_a = fainted_from(&hp.a);
    let next_b = fainted_from(&hp.b);

    if next_a != *fainted_a || next_b != *fainted_b {
        *fainted_a = next_a;
        *fainted_b = next_b;
        *image_url = image_host::host(
            ctx,
            &scene.render(&log.team_a, &log.team_b, fainted_a, fainted_b),
        )
        .await;
    }
}

/// An `opponent` means PvP; without one it is PvE at the chosen difficulty.
///
/// The old prefix version had to guess which of those a bare word meant, and
/// reject anything it did not recognise. Discord now types the options for us:
/// `opponent` is a real user picker, and `difficulty` is a fixed choice list, so
/// neither can arrive malformed.
pub async fn slash_luckybattle(inv: &Invocation<'_>) -> serenity::Result<()> {
    let opponent = inv.user_arg("opponent");

    // A PvP challenge has to be public -- the person being challenged needs to
    // see it to accept. The PvE prompt is only ever addressed to the caller, so
    // it stays private and keeps the channel clear; the battle itself is still
    // posted for everyone.
    inv.defer(opponent.is_none()).await?;

    let guild = match require_guild(inv).await {
        Some(g) => g,
        None => return Ok(()),
    };

    let guild_id = i64::from(guild);
    let challenger_id = i64::from(inv.user_id());

    match opponent {
        Some(opponent_id) => pvp(inv, guild, guild_id, challenger_id, opponent_id).await,
        None => {
            let practice = inv
                .string("mode")
                .map(|m| m.eq_ignore_ascii_case("practice"))
                .unwrap_or(false);
            pve(inv, guild, guild_id, challenger_id, practice).await
        }
    }
}

/// PvE announces the trainer first, then fights on confirmation.
///
/// The server derives the trainer from the date and today's battle count rather
/// than the battle's random seed, so the name shown here is the one that turns
/// up in the fight. That gap is the whole point: it is where the player gets to
/// answer a Water gym with `/luckyteam` instead of finding out too late.
async fn pve(
    inv: &Invocation<'_>,
    guild: GuildId,
    guild_id: i64,
    challenger_id: i64,
    practice: bool,
) -> serenity::Result<()> {
    let ctx = inv.ctx;

    let trainer = match battle_api::get_next_trainer(guild_id, challenger_id).await {
        Ok(t) => t,
        Err(e) => return inv.fail(e).await,
    };

    // Surface the team they would currently field, so "adjust it" is an
    // informed decision rather than a guess.
    let team_line = match battle_api::get_team(challenger_id).await {
        Ok(team) => team
            .picks
            .iter()
            .map(|p| {
                let name = format_for_display(&p.name);
                if p.shiny {
                    format!("\u{2728} {}", name)
                } else {
                    name
                }
            })
            .collect::<Vec<String>>()
            .join(", "),
        Err(e) => return inv.fail(e).await,
    };

    let theme_line = match &trainer.theme {
        Some(t) => format!("Specialises in **{}** types.", title_case_word(t)),
        None => "Runs a mixed team.".to_string(),
    };

    let field_line = battle_api::get_battlefield()
        .await
        .map(|b| format!("{} **{}** \u{2014} {}", b.emoji, b.name, b.summary))
        .unwrap_or_default();

    let action_row = (*CreateActionRow::default()
        .add_button(
            (*CreateButton::default()
                .custom_id("start_pve")
                .label("Fight")
                .style(ButtonStyle::Success))
            .clone(),
        )
        .add_button(
            (*CreateButton::default()
                .custom_id("cancel_pve")
                .label("Cancel")
                .style(ButtonStyle::Secondary))
            .clone(),
        ))
    .clone();

    // Say up front when this one is for fun, so nobody plays a serious battle
    // by accident or a practice one thinking it counted.
    let title = if practice {
        format!(
            "\u{1F94A} Practice bout with {} \u{2014} doesn't count",
            trainer.name
        )
    } else {
        format!("\u{2694}\u{FE0F} {} wants to battle!", trainer.name)
    };

    let footer = if practice {
        "Practice \u{2014} the result won't reach the leaderboard, and it won't use up one of today's counted battles."
    } else {
        "Change your line-up with /luckyteam, then Fight. This trainer waits until you do."
    };

    inv.command
        .edit_original_interaction_response(&ctx.http, |r| {
            r.embed(|e| {
                e.title(title)
                    .description(format!("{}\n\n{}", theme_line, field_line))
                    .field("Your team", team_line, false)
                    .footer(|f| f.text(footer))
            })
            .components(|c| c.add_action_row(action_row))
        })
        .await?;

    let prompt = inv.command.get_interaction_response(&ctx.http).await?;
    let challenger_name = display_name(ctx, guild, inv.user_id()).await;

    while let Some(interaction) = prompt
        .await_component_interaction(ctx)
        .timeout(Duration::from_secs(CHALLENGE_TIMEOUT_SECS))
        .await
    {
        if interaction.user.id != inv.user_id() {
            let _ = interaction
                .create_interaction_response(&ctx.http, |r| {
                    r.kind(InteractionResponseType::ChannelMessageWithSource)
                        .interaction_response_data(|d| {
                            d.ephemeral(true)
                                .content("This battle isn't yours to start.")
                        })
                })
                .await;
            continue;
        }

        if interaction.data.custom_id == "cancel_pve" {
            let _ = interaction
                .create_interaction_response(&ctx.http, |r| {
                    r.kind(InteractionResponseType::UpdateMessage)
                        .interaction_response_data(|d| {
                            d.embed(|e| {
                                e.title("\u{1F6B6} Walked Away")
                                    .description(format!(
                                        "{} decided not to battle {}.",
                                        inv.user().mention(),
                                        trainer.name
                                    ))
                            })
                            .components(|c| c)
                        })
                })
                .await;
            return Ok(());
        }

        if interaction.data.custom_id != "start_pve" {
            continue;
        }

        // Running the battle means an HTTP round trip and a simulation, so
        // acknowledge first and edit through the interaction endpoint.
        interaction
            .create_interaction_response(&ctx.http, |r| {
                r.kind(InteractionResponseType::DeferredUpdateMessage)
            })
            .await?;

        let _ = interaction
            .edit_original_interaction_response(&ctx.http, |r| {
                r.embed(|e| {
                    e.title(format!("\u{2694}\u{FE0F} {} accepts the challenge!", trainer.name))
                        .description(theme_line.clone())
                })
                .components(|c| c)
            })
            .await;

        let req = BattleRequest {
            guild_id,
            battle_type: "pve".to_string(),
            challenger_id,
            challenger_team: None,
            opponent_id: None,
            opponent_team: None,
            practice: Some(practice),
        };

        let outcome = match battle_api::run_battle(&req).await {
            Ok(o) => o,
            Err(e) => {
                let _ = inv
                    .command
                    .channel_id
                    .say(&ctx.http, format!("{} {}", inv.user().mention(), e))
                    .await;
                return Ok(());
            }
        };

        let opponent = outcome
            .battle
            .opponent_name
            .clone()
            .unwrap_or_else(|| trainer.name.clone());

        return play_back(
            ctx,
            inv.command.channel_id,
            &outcome,
            &challenger_name,
            &opponent,
        )
        .await;
    }

    // Nobody answered. Strip the buttons rather than leave them dead.
    let _ = inv
        .command
        .edit_original_interaction_response(&ctx.http, |r| {
            r.embed(|e| {
                e.title("\u{23F0} Battle Expired")
                    .description(format!("{} wandered off.", trainer.name))
            })
            .components(|c| c)
        })
        .await;

    Ok(())
}

/// Title-cases a single lowercase word, for type names coming off the API.
fn title_case_word(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// PvP shows the challenger's lead only. The defender can read it and set their
/// own team with `/luckyteam` before accepting -- the counter-pick falls out of
/// commands that already exist, with no extra UI.
async fn pvp(
    inv: &Invocation<'_>,
    guild: GuildId,
    guild_id: i64,
    challenger_id: i64,
    opponent_id: UserId,
) -> serenity::Result<()> {
    let ctx = inv.ctx;
    let challenger = inv.user().clone();

    if opponent_id == challenger.id {
        return inv.fail("You can't battle yourself, silly!").await;
    }

    let lead = match battle_api::get_team(challenger_id).await {
        Ok(team) => team.picks.first().cloned(),
        Err(e) => return inv.fail(e).await,
    };

    let lead_text = match &lead {
        Some(p) => {
            let name = format_for_display(&p.name);
            if p.shiny {
                format!("\u{2728} Shiny {}", name)
            } else {
                name
            }
        }
        None => "an unknown pokemon".to_string(),
    };

    let battlefield = battle_api::get_battlefield().await;
    let field_line = battlefield
        .map(|b| format!("{} **{}** \u{2014} {}", b.emoji, b.name, b.summary))
        .unwrap_or_default();

    let action_row = (*CreateActionRow::default()
        .add_button(
            (*CreateButton::default()
                .custom_id("accept_battle")
                .label("Accept")
                .style(ButtonStyle::Success))
            .clone(),
        )
        .add_button(
            (*CreateButton::default()
                .custom_id("decline_battle")
                .label("Decline")
                .style(ButtonStyle::Danger))
            .clone(),
        ))
    .clone();

    inv.command
        .edit_original_interaction_response(&ctx.http, |r| {
            r.embed(|e| {
                e.title("\u{2694}\u{FE0F} Luckybattle Challenge!")
                    .description(format!(
                        "{} has challenged {} to a 3v3 battle.\n\n{}",
                        challenger.mention(),
                        opponent_id.mention(),
                        field_line
                    ))
                    .field("Their lead", lead_text, true)
                    .footer(|f| {
                        f.text("Set your own line-up with /luckyteam before you accept.")
                    })
            })
            .components(|c| c.add_action_row(action_row))
        })
        .await?;

    let challenge = inv.command.get_interaction_response(&ctx.http).await?;

    while let Some(interaction) = challenge
        .await_component_interaction(ctx)
        .timeout(Duration::from_secs(CHALLENGE_TIMEOUT_SECS))
        .await
    {
        if interaction.user.id != challenger.id && interaction.user.id != opponent_id {
            // Answer bystanders too. Skipping the acknowledgement entirely is
            // what produces "the application didn't respond in time" for them.
            let _ = interaction
                .create_interaction_response(&ctx.http, |r| {
                    r.kind(InteractionResponseType::ChannelMessageWithSource)
                        .interaction_response_data(|d| {
                            d.ephemeral(true)
                                .content("This challenge isn't yours to answer.")
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

        let custom_id = interaction.data.custom_id.as_str();

        if custom_id == "accept_battle" && interaction.user.id == opponent_id {
            let req = BattleRequest {
                guild_id,
                battle_type: "pvp".to_string(),
                challenger_id,
                challenger_team: None,
                opponent_id: Some(i64::from(opponent_id)),
                opponent_team: None,
                practice: None,
            };

            // Clear the challenge buttons; results land in their own messages.
            let _ = interaction
                .edit_original_interaction_response(&ctx.http, |r| {
                    r.embed(|em| {
                        em.title("\u{2694}\u{FE0F} Challenge Accepted!")
                            .description(format!(
                                "{} accepted {}'s challenge.",
                                opponent_id.mention(),
                                challenger.mention()
                            ))
                    })
                    .components(|c| c)
                })
                .await;

            let challenger_name = display_name(ctx, guild, challenger.id).await;
            let opponent_name = display_name(ctx, guild, opponent_id).await;

            let outcome = match battle_api::run_battle(&req).await {
                Ok(o) => o,
                Err(e) => {
                    let _ = inv
                        .command
                        .channel_id
                        .say(&ctx.http, format!("{} {}", challenger.mention(), e))
                        .await;
                    return Ok(());
                }
            };

            return play_back(
                ctx,
                inv.command.channel_id,
                &outcome,
                &challenger_name,
                &opponent_name,
            )
            .await;
        } else if custom_id == "decline_battle" {
            let who = interaction.user.id;
            let _ = interaction
                .edit_original_interaction_response(&ctx.http, |r| {
                    r.embed(|em| {
                        em.title("\u{274C} Challenge Declined")
                            .description(format!("{} called it off. \u{1F622}", who.mention()))
                    })
                    .components(|c| c)
                })
                .await;
            return Ok(());
        }
    }

    // The challenge lapsed. Strip the buttons so a click an hour later gets a
    // dead control rather than an "application didn't respond" error.
    let _ = inv
        .command
        .edit_original_interaction_response(&ctx.http, |r| {
            r.embed(|e| {
                e.title("\u{23F0} Challenge Expired")
                    .description(format!(
                        "{} didn't answer {}'s challenge in time.",
                        opponent_id.mention(),
                        challenger.mention()
                    ))
            })
            .components(|c| c)
        })
        .await;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hp_bar_endpoints() {
        assert_eq!(hp_bar(100), "\u{2588}".repeat(10));
        assert_eq!(hp_bar(0), "\u{2591}".repeat(10));
    }

    #[test]
    fn a_living_pokemon_never_shows_an_empty_bar() {
        for pct in 1..=4 {
            assert!(
                hp_bar(pct).starts_with('\u{2588}'),
                "{}% rendered as an empty bar",
                pct
            );
        }
    }

    #[test]
    fn hp_bar_is_always_ten_cells() {
        for pct in 0..=100 {
            assert_eq!(hp_bar(pct).chars().count(), 10, "at {}%", pct);
        }
    }

    #[test]
    fn hp_bar_clamps_out_of_range_input() {
        assert_eq!(hp_bar(150).chars().count(), 10);
        assert_eq!(hp_bar(-20), "\u{2591}".repeat(10));
    }

    fn mon(pokemon_id: i64, name: &str, shiny: bool) -> MonSnapshot {
        MonSnapshot {
            slot: 0,
            pokemon_id,
            name: name.to_string(),
            shiny,
            type_1: "normal".to_string(),
            type_2: None,
            max_hp: 160,
        }
    }

    /// Renders a real battle image to disk for eyeballing, mid-battle with some
    /// of the line-up already down. Ignored by default because it depends on
    /// `resources/` being present and produces a file rather than an assertion.
    #[test]
    #[ignore]
    fn preview_battle_image() {
        let team_a = vec![
            mon(6, "charizard", false),
            mon(9, "blastoise", false),
            mon(150, "mewtwo", false),
        ];
        let team_b = vec![
            mon(25, "pikachu", true),
            mon(94, "gengar", false),
            mon(213, "shuckle", false),
        ];

        let scene = BattleScene::new();
        let img = scene.render(
            &team_a,
            &team_b,
            &[true, false, false],
            &[true, true, false],
        );

        assert_eq!(img.width(), 400);
        assert_eq!(img.height(), 300);

        let path = std::env::var("BATTLE_PREVIEW_PATH")
            .unwrap_or_else(|_| "battle_preview.png".to_string());
        img.save(&path).expect("could not write preview");
        println!("wrote {}", path);
    }

    #[test]
    fn fainted_slots_are_read_from_zero_percent() {
        assert_eq!(fainted_from(&[100, 0, 43]), vec![false, true, false]);
        assert_eq!(fainted_from(&[]), Vec::<bool>::new());
    }

    /// The server promises a living pokemon never reports 0%, which is what
    /// lets this be an equality check rather than a threshold.
    #[test]
    fn one_percent_is_still_alive() {
        assert_eq!(fainted_from(&[1]), vec![false]);
    }

    #[test]
    fn desaturate_keeps_transparency() {
        let mut img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(2, 1);
        img.put_pixel(0, 0, Rgba([200, 50, 25, 255]));
        img.put_pixel(1, 0, Rgba([200, 50, 25, 0]));

        desaturate(&mut img);

        let opaque = img.get_pixel(0, 0).0;
        assert_eq!(opaque[0], opaque[1]);
        assert_eq!(opaque[1], opaque[2]);
        assert_eq!(opaque[3], 255, "opaque pixels stay opaque");

        assert_eq!(
            img.get_pixel(1, 0).0[3],
            0,
            "transparent pixels must stay transparent, or the sprite becomes a grey box"
        );
    }

    #[test]
    fn the_two_backgrounds_always_differ() {
        let mut rng = rand::thread_rng();
        for _ in 0..20_000 {
            let (top, bottom) = pick_two_backgrounds(&mut rng);
            assert_ne!(top, bottom, "both teams got the same background");
            assert!((1..=BACKGROUND_COUNT).contains(&top), "top out of range: {}", top);
            assert!(
                (1..=BACKGROUND_COUNT).contains(&bottom),
                "bottom out of range: {}",
                bottom
            );
        }
    }

    #[test]
    fn every_background_can_be_drawn() {
        // The shift trick is easy to get subtly wrong at the ends of the range,
        // so confirm nothing is unreachable in either slot.
        let mut rng = rand::thread_rng();
        let mut seen_top = vec![false; (BACKGROUND_COUNT + 1) as usize];
        let mut seen_bottom = vec![false; (BACKGROUND_COUNT + 1) as usize];

        for _ in 0..20_000 {
            let (top, bottom) = pick_two_backgrounds(&mut rng);
            seen_top[top as usize] = true;
            seen_bottom[bottom as usize] = true;
        }

        for i in 1..=BACKGROUND_COUNT as usize {
            assert!(seen_top[i], "bg{} never appeared on top", i);
            assert!(seen_bottom[i], "bg{} never appeared on the bottom", i);
        }
    }

    #[test]
    fn short_battles_show_every_turn() {
        assert_eq!(frame_indices(5), vec![0, 1, 2, 3, 4]);
        assert_eq!(frame_indices(0), Vec::<usize>::new());
    }

    /// The closing edit owns the final frame. If the playback loop drew it too,
    /// two edits would land back to back and "Battle in progress..." could be
    /// the last thing written.
    #[test]
    fn playback_stops_before_the_final_frame() {
        for total in [0usize, 1, 2, 5, 13, 30, 50] {
            let frames = frame_indices(total);
            let playback = frames.len().saturating_sub(1);

            assert!(
                playback <= frames.len(),
                "playback range must stay in bounds for {} turns",
                total
            );

            if let Some(last) = frames.last() {
                assert_eq!(*last, total - 1, "last frame is the final event");
                assert!(
                    !frames[..playback].contains(last),
                    "the loop must not draw the final frame ({} turns)",
                    total
                );
            }
        }
    }

    #[test]
    fn long_battles_are_capped_and_end_on_the_knockout() {
        for total in [13usize, 30, 50, 120] {
            let frames = frame_indices(total);
            assert!(
                frames.len() <= MAX_FRAMES,
                "{} turns produced {} frames",
                total,
                frames.len()
            );
            assert_eq!(
                *frames.last().unwrap(),
                total - 1,
                "last frame must be the final event"
            );
            assert!(
                frames.windows(2).all(|w| w[0] < w[1]),
                "frames must advance monotonically"
            );
        }
    }
}
