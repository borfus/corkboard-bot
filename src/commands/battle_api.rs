//! Wire types and HTTP helpers for the luckybattle endpoints.
//!
//! The bot is a renderer: it resolves Discord ids to names, composites sprites
//! and plays back a log. Every rule -- team validation, the simulation, the
//! battlefield, who won -- lives on the server, so nothing here recomputes
//! anything it was handed.

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const API_BASE: &str = "http://localhost:8000/api/v1";

/// One chosen pokemon. `123` and `123s` in command arguments both land here.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub struct TeamPick {
    pub pokemon_id: i64,
    pub shiny: bool,
}

/// Mirrors the server payload in full even where the bot does not read every
/// field yet, so the wire contract stays visible in one place.
#[allow(dead_code)]
#[derive(Deserialize, Debug, Clone)]
pub struct MonSnapshot {
    pub slot: usize,
    pub pokemon_id: i64,
    pub name: String,
    pub shiny: bool,
    pub type_1: String,
    pub type_2: Option<String>,
    pub max_hp: i64,
}

#[derive(Deserialize, Debug, Clone)]
pub struct HpSnapshot {
    pub a: Vec<i64>,
    pub b: Vec<i64>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct TurnEvent {
    #[allow(dead_code)]
    pub n: usize,
    #[allow(dead_code)]
    pub actor: String,
    /// Pre-rendered by the server so playback needs no knowledge of the rules.
    pub text: String,
    pub hp_after: HpSnapshot,
}

#[derive(Deserialize, Debug, Clone)]
pub struct BattleLog {
    #[allow(dead_code)]
    pub battlefield: String,
    pub battlefield_name: String,
    pub battlefield_emoji: String,
    pub battlefield_summary: String,
    pub team_a: Vec<MonSnapshot>,
    pub team_b: Vec<MonSnapshot>,
    pub turns: Vec<TurnEvent>,
    pub winner: String,
    #[allow(dead_code)]
    pub total_turns: usize,
    pub decided_on_hp: bool,
}

/// As above: the stored battle row, kept whole for the eventual replay command.
#[allow(dead_code)]
#[derive(Deserialize, Debug, Clone)]
pub struct BattleRecord {
    pub id: String,
    pub battle_type: Option<String>,
    pub challenger_id: Option<i64>,
    pub opponent_id: Option<i64>,
    pub opponent_name: Option<String>,
    pub winner_id: Option<i64>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct BattleOutcome {
    pub battle: BattleRecord,
    pub log: BattleLog,
    pub counted: bool,
    #[allow(dead_code)]
    pub opponent_label: String,
}

#[derive(Serialize, Debug)]
pub struct BattleRequest {
    pub guild_id: i64,
    pub battle_type: String,
    pub challenger_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub challenger_team: Option<Vec<TeamPick>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opponent_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opponent_team: Option<Vec<TeamPick>>,
    /// PvE only. Same fight, just kept off the leaderboard.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub practice: Option<bool>,
}

/// A pick as the server returns it, with the species name already attached.
#[derive(Deserialize, Debug, Clone)]
pub struct TeamEntry {
    pub pokemon_id: i64,
    pub shiny: bool,
    pub name: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct TeamResponse {
    pub picks: Vec<TeamEntry>,
    pub saved: bool,
}

#[derive(Serialize, Debug)]
pub struct SaveTeamRequest {
    pub user_id: i64,
    pub picks: Vec<TeamPick>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct LeaderboardRow {
    pub user_id: i64,
    pub wins: i64,
    pub losses: i64,
}

#[derive(Deserialize, Debug, Clone)]
pub struct LeaderboardResponse {
    pub entries: Vec<LeaderboardRow>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct BattlefieldView {
    pub name: String,
    pub emoji: String,
    pub summary: String,
}

/// The NPC a player will face next, announced before they commit.
#[derive(Deserialize, Debug, Clone)]
pub struct TrainerView {
    pub name: String,
    /// `None` for the mixed-team trainer.
    pub theme: Option<String>,
}

/// Pulls the server's `{"error": "..."}` body out of a non-2xx response so the
/// bot can show the user what actually went wrong instead of a status code.
async fn read_error(resp: reqwest::Response) -> String {
    let fallback = "The server rejected that request.".to_string();
    match resp.json::<Value>().await {
        Ok(v) => v
            .get("error")
            .and_then(|e| e.as_str())
            .map(|s| s.to_string())
            .unwrap_or(fallback),
        Err(_) => fallback,
    }
}

pub async fn run_battle(req: &BattleRequest) -> Result<BattleOutcome, String> {
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/luckymon-battle", API_BASE))
        .json(req)
        .send()
        .await
        .map_err(|e| format!("Could not reach the server: {}", e))?;

    if !resp.status().is_success() {
        return Err(read_error(resp).await);
    }

    resp.json::<BattleOutcome>()
        .await
        .map_err(|e| format!("Could not read the battle result: {}", e))
}

/// The team a user would field right now: their saved team if still valid,
/// otherwise the server's auto-pick.
pub async fn get_team(user_id: i64) -> Result<TeamResponse, String> {
    let resp = reqwest::get(format!("{}/luckymon-team/user-id/{}", API_BASE, user_id))
        .await
        .map_err(|e| format!("Could not reach the server: {}", e))?;

    if !resp.status().is_success() {
        return Err(read_error(resp).await);
    }

    resp.json::<TeamResponse>()
        .await
        .map_err(|e| format!("Could not read that team: {}", e))
}

pub async fn save_team(user_id: i64, picks: Vec<TeamPick>) -> Result<TeamResponse, String> {
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/luckymon-team", API_BASE))
        .json(&SaveTeamRequest { user_id, picks })
        .send()
        .await
        .map_err(|e| format!("Could not reach the server: {}", e))?;

    if !resp.status().is_success() {
        return Err(read_error(resp).await);
    }

    resp.json::<TeamResponse>()
        .await
        .map_err(|e| format!("Could not read the saved team: {}", e))
}

pub async fn get_leaderboard(
    guild_id: i64,
    battle_type: &str,
    limit: i64,
) -> Result<LeaderboardResponse, String> {
    let resp = reqwest::get(format!(
        "{}/luckymon-battle/leaderboard/{}?battle_type={}&limit={}",
        API_BASE, guild_id, battle_type, limit
    ))
    .await
    .map_err(|e| format!("Could not reach the server: {}", e))?;

    if !resp.status().is_success() {
        return Err(read_error(resp).await);
    }

    resp.json::<LeaderboardResponse>()
        .await
        .map_err(|e| format!("Could not read the leaderboard: {}", e))
}

// A rank lookup used to live here. Ranking moved into `luckyboard`, which
// numbers positions only after dropping players who have left the guild -- a
// rank fetched from the server would not match the list on screen.

/// Who this player faces next. Stable until they actually battle, so the
/// announcement and the fight cannot disagree.
pub async fn get_next_trainer(guild_id: i64, user_id: i64) -> Result<TrainerView, String> {
    let resp = reqwest::get(format!(
        "{}/luckymon-battle/next-trainer/{}/{}",
        API_BASE, guild_id, user_id
    ))
    .await
    .map_err(|e| format!("Could not reach the server: {}", e))?;

    if !resp.status().is_success() {
        return Err(read_error(resp).await);
    }

    resp.json::<TrainerView>()
        .await
        .map_err(|e| format!("Could not read the trainer: {}", e))
}

pub async fn get_battlefield() -> Option<BattlefieldView> {
    reqwest::get(format!("{}/luckymon-battlefield", API_BASE))
        .await
        .ok()?
        .json::<BattlefieldView>()
        .await
        .ok()
}

/// Parses a team argument the way `/luckytrade` does: `123`, or `123s` for the
/// shiny copy.
pub fn parse_pick(arg: &str) -> Option<TeamPick> {
    let arg = arg.trim();
    if arg.is_empty() {
        return None;
    }

    let (digits, shiny) = if arg.ends_with('s') || arg.ends_with('S') {
        (&arg[..arg.len() - 1], true)
    } else {
        (arg, false)
    };

    digits
        .parse::<i64>()
        .ok()
        .filter(|id| *id > 0)
        .map(|pokemon_id| TeamPick { pokemon_id, shiny })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_and_shiny_picks() {
        assert_eq!(
            parse_pick("143"),
            Some(TeamPick { pokemon_id: 143, shiny: false })
        );
        assert_eq!(
            parse_pick("143s"),
            Some(TeamPick { pokemon_id: 143, shiny: true })
        );
        assert_eq!(
            parse_pick(" 6S "),
            Some(TeamPick { pokemon_id: 6, shiny: true })
        );
    }

    #[test]
    fn rejects_nonsense() {
        assert_eq!(parse_pick(""), None);
        assert_eq!(parse_pick("s"), None);
        assert_eq!(parse_pick("abc"), None);
        assert_eq!(parse_pick("0"), None);
        assert_eq!(parse_pick("-5"), None);
    }
}
