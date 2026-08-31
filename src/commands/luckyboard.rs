//! `/luckyboard` -- per-guild battle standings, PvP and PvE on one message.
//!
//! Wins are scoped to the guild the command is run in: collections are global,
//! but a leaderboard only means something among people who can see each other.

use std::time::Duration;

use serenity::builder::{CreateActionRow, CreateButton};
use serenity::model::application::interaction::InteractionResponseType;
use serenity::model::id::{GuildId, UserId};
use serenity::model::prelude::component::ButtonStyle;
use serenity::prelude::*;

use super::battle_api;
use crate::slash::{present_member_name, require_guild, Invocation};

/// How many rows end up on screen.
const TOP_N: usize = 10;

/// How many rows to ask the server for.
///
/// More than are shown, because some of them will belong to people who have
/// left the server since they last played. Walking a bigger window means the
/// board still fills up when a few names drop out, and the walk stops early
/// once it has what it needs -- so the extra rows usually cost nothing.
const FETCH_ROWS: i64 = 60;

const VIEW_TIMEOUT_SECS: u64 = 180;

/// One row of the board, after we know the player is still here.
struct Standing {
    position: usize,
    name: String,
    wins: i64,
    losses: i64,
    is_caller: bool,
}

/// Walks the global standings, keeping only people still in this server.
///
/// The server totals wins across every guild and narrows to players who have
/// battled here; this drops the ones who have since left, so a departed player
/// no longer occupies a slot as "Unknown (1234...)". Positions are numbered after
/// that filter, which is why they are counted here rather than in SQL -- a rank
/// worked out server-side would not match the list on screen.
///
/// Stops as soon as it has a full board *and* has placed the caller, so the
/// larger fetch window normally costs nothing.
async fn collect_standings(
    ctx: &Context,
    guild_id: GuildId,
    caller_id: i64,
    rows: &[battle_api::LeaderboardRow],
) -> Vec<Standing> {
    let mut standings: Vec<Standing> = Vec::new();
    let mut position = 0usize;
    let mut caller_placed = false;

    for row in rows.iter() {
        if standings.len() >= TOP_N && caller_placed {
            break;
        }

        let name = match present_member_name(ctx, guild_id, UserId(row.user_id as u64)).await {
            Some(n) => n,
            None => continue,
        };

        position += 1;
        let is_caller = row.user_id == caller_id;
        if is_caller {
            caller_placed = true;
        }

        // Past the visible board only the caller's own row is worth keeping.
        if standings.len() < TOP_N || is_caller {
            standings.push(Standing {
                position,
                name,
                wins: row.wins,
                losses: row.losses,
                is_caller,
            });
        }
    }

    standings
}

fn truncate(name: &str) -> String {
    if name.chars().count() > 16 {
        name.chars().take(16).collect()
    } else {
        name.to_string()
    }
}

/// Builds the board body. Returns the rendered text; the caller decides how to
/// present a failure.
async fn render_board(
    ctx: &Context,
    guild_id: GuildId,
    caller_id: i64,
    battle_type: &str,
) -> String {
    let board =
        match battle_api::get_leaderboard(i64::from(guild_id), battle_type, FETCH_ROWS).await {
            Ok(b) => b,
            Err(e) => return format!("Could not load the leaderboard: {}", e),
        };

    let standings = collect_standings(ctx, guild_id, caller_id, &board.entries).await;

    if standings.is_empty() {
        return format!(
            "No {} battles yet from anyone still in this server. Be the first \u{2014} `/luckybattle`",
            battle_type.to_uppercase()
        );
    }

    let mut lines: Vec<String> = Vec::new();

    for entry in standings.iter().take(TOP_N) {
        let medal = match entry.position {
            1 => "\u{1F947}",
            2 => "\u{1F948}",
            3 => "\u{1F949}",
            _ => "\u{2003}",
        };
        lines.push(format!(
            "{} `{:>2}. {:<16} {:>3}W - {:>3}L`",
            medal,
            entry.position,
            truncate(&entry.name),
            entry.wins,
            entry.losses
        ));
    }

    let mut body = lines.join("\n");

    // Showing the caller their own standing is the thing that brings people
    // back when they are nowhere near the top.
    let caller = standings.iter().find(|s| s.is_caller);
    let shown = standings.iter().take(TOP_N).any(|s| s.is_caller);

    if !shown {
        match caller {
            Some(entry) => body.push_str(&format!(
                "\n\n`\u{2014}\u{2014}` Your rank: **#{}** ({}W - {}L)",
                entry.position, entry.wins, entry.losses
            )),
            None => body.push_str("\n\n`\u{2014}\u{2014}` You haven't battled here yet."),
        }
    }

    body
}

fn title_for(battle_type: &str) -> &'static str {
    if battle_type == "pve" {
        "\u{1F3C6} PvE Leaderboard \u{2014} Top 10"
    } else {
        "\u{1F3C6} PvP Leaderboard \u{2014} Top 10"
    }
}

/// The inactive mode gets the highlighted button, so the label reads as "go
/// here next" rather than "you are here".
fn buttons(active: &str) -> CreateActionRow {
    let pvp_style = if active == "pvp" {
        ButtonStyle::Secondary
    } else {
        ButtonStyle::Primary
    };
    let pve_style = if active == "pve" {
        ButtonStyle::Secondary
    } else {
        ButtonStyle::Primary
    };

    (*CreateActionRow::default()
        .add_button(
            (*CreateButton::default()
                .custom_id("board_pvp")
                .label("PvP")
                .style(pvp_style)
                .disabled(active == "pvp"))
            .clone(),
        )
        .add_button(
            (*CreateButton::default()
                .custom_id("board_pve")
                .label("PvE")
                .style(pve_style)
                .disabled(active == "pve"))
            .clone(),
        ))
    .clone()
}

pub async fn slash_luckyboard(inv: &Invocation<'_>) -> serenity::Result<()> {
    inv.defer(false).await?;

    let guild_id = match require_guild(inv).await {
        Some(g) => g,
        None => return Ok(()),
    };

    let ctx = inv.ctx;
    let caller_id = i64::from(inv.user_id());
    let mut mode = "pvp".to_string();
    let body = render_board(ctx, guild_id, caller_id, &mode).await;

    inv.command
        .edit_original_interaction_response(&ctx.http, |r| {
            r.embed(|e| {
                e.title(title_for(&mode))
                    .description(body)
                    .footer(|f| f.text("Wins counted everywhere; showing this server's players."))
            })
            .components(|c| c.add_action_row(buttons(&mode)))
        })
        .await?;

    // The collector needs the message the components are attached to, which for
    // an interaction is the response itself.
    let board_msg = inv.command.get_interaction_response(&ctx.http).await?;

    while let Some(interaction) = board_msg
        .await_component_interaction(ctx)
        .timeout(Duration::from_secs(VIEW_TIMEOUT_SECS))
        .await
    {
        // Re-rendering costs several HTTP calls plus a member lookup per row,
        // so acknowledge before doing the work, then answer through the
        // interaction endpoint we just promised.
        interaction
            .create_interaction_response(&ctx.http, |r| {
                r.kind(InteractionResponseType::DeferredUpdateMessage)
            })
            .await?;

        mode = match interaction.data.custom_id.as_str() {
            "board_pve" => "pve".to_string(),
            _ => "pvp".to_string(),
        };

        let body = render_board(ctx, guild_id, i64::from(interaction.user.id), &mode).await;

        let _ = interaction
            .edit_original_interaction_response(&ctx.http, |r| {
                r.embed(|e| {
                    e.title(title_for(&mode))
                        .description(body)
                        .footer(|f| f.text("Wins counted everywhere; showing this server's players."))
                })
                .components(|c| c.add_action_row(buttons(&mode)))
            })
            .await;
    }

    // Leave the final view readable but inert once nobody is interacting.
    let _ = inv
        .command
        .edit_original_interaction_response(&ctx.http, |r| r.components(|c| c))
        .await;

    Ok(())
}
