//! `/luckyteam` -- view or set the line-up you bring to a luckybattle.
//!
//! Always ephemeral. A visible team is a scouting report: an opponent who can
//! read your line-up before accepting a challenge has already won the
//! interesting part of the battle. As a slash command the arguments are never
//! posted publicly either, so "6 9 143" no longer sits in the channel for
//! anyone to read.
//!
//! Saving is optional. Without a saved team the server auto-picks, so this
//! exists for players who want to answer a challenger's lead, not as a step
//! everyone has to complete first.

use serenity::builder::CreateEmbed;

use super::battle_api::{self, TeamEntry, TeamPick};
use super::luckymon::format_for_display;
use crate::slash::Invocation;

const MAX_TEAM: usize = 3;

fn render_entry(index: usize, entry: &TeamEntry) -> String {
    let name = format_for_display(&entry.name);
    let name = if entry.shiny {
        format!("\u{2728} Shiny {}", name)
    } else {
        name
    };
    format!("`{}.` {}  \u{2014}  #{}", index + 1, name, entry.pokemon_id)
}

fn render_team(entries: &[TeamEntry]) -> String {
    if entries.is_empty() {
        return "_No pokemon available._".to_string();
    }
    entries
        .iter()
        .enumerate()
        .map(|(i, e)| render_entry(i, e))
        .collect::<Vec<String>>()
        .join("\n")
}

pub async fn slash_luckyteam(inv: &Invocation<'_>) -> serenity::Result<()> {
    inv.defer(true).await?;

    let user_id = i64::from(inv.user_id());

    match inv.string("team") {
        Some(raw) if !raw.trim().is_empty() => save(inv, user_id, &raw).await,
        _ => show(inv, user_id).await,
    }
}

async fn save(inv: &Invocation<'_>, user_id: i64, raw: &str) -> serenity::Result<()> {
    let tokens: Vec<&str> = raw.split_whitespace().collect();

    if tokens.len() > MAX_TEAM {
        return inv
            .fail(format!(
                "A team is at most {} pokemon. You gave {}.",
                MAX_TEAM,
                tokens.len()
            ))
            .await;
    }

    let mut picks: Vec<TeamPick> = Vec::new();
    for token in tokens.iter() {
        match battle_api::parse_pick(token) {
            Some(p) => picks.push(p),
            None => {
                return inv
                    .fail(format!(
                        "`{}` isn't a pokedex number. Try `6 9 143`, or `143s` for a shiny.",
                        token
                    ))
                    .await
            }
        }
    }

    match battle_api::save_team(user_id, picks).await {
        Ok(team) => {
            let field_line = battle_api::get_battlefield()
                .await
                .map(|b| format!("\n\n{} **{}** \u{2014} {}", b.emoji, b.name, b.summary))
                .unwrap_or_default();

            let mut embed = CreateEmbed::default();
            embed
                .title("\u{2705} Team Saved")
                .description(format!("{}{}", render_team(&team.picks), field_line))
                .footer(|f| f.text("Used for every luckybattle until you change it."));

            inv.embed(embed).await
        }
        Err(e) => inv.fail(e).await,
    }
}

async fn show(inv: &Invocation<'_>, user_id: i64) -> serenity::Result<()> {
    let team = match battle_api::get_team(user_id).await {
        Ok(t) => t,
        Err(e) => return inv.fail(e).await,
    };

    let field_line = battle_api::get_battlefield()
        .await
        .map(|b| format!("{} **{}**\n{}", b.emoji, b.name, b.summary))
        .unwrap_or_else(|| "_Could not read today's battlefield._".to_string());

    let source = if team.saved {
        "Your saved team."
    } else {
        "Auto-picked \u{2014} set your own with /luckyteam team: 6 9 143"
    };

    let mut embed = CreateEmbed::default();
    embed
        .title("Your Luckybattle Team")
        .description(render_team(&team.picks))
        .field("Today's battlefield", field_line, false)
        .footer(|f| f.text(source));

    inv.embed(embed).await
}
