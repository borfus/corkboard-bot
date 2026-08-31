extern crate serde;
extern crate serde_json;

use std::collections::HashMap;

use chrono::NaiveDateTime;
use reqwest::header::CONTENT_TYPE;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serenity::builder::CreateEmbed;
use serenity::model::Timestamp;
use uuid::Uuid;

use crate::pacific;
use crate::slash::{require_guild, Invocation};
use crate::validation::validation;

const BOARD_IMAGE: &str = "./resources/cork-board.png";

/// What a person types.
const INPUT_FMT: &str = "%m/%d/%Y %-I:%M%p";
/// What the API speaks.
const API_FMT: &str = "%Y-%m-%dT%H:%M:%S%.f";

#[derive(Serialize, Deserialize, Debug)]
pub struct Event {
    pub id: Uuid,
    pub guild_id: i64,
    pub title: String,
    pub url: String,
    pub description: String,
    pub start_date: NaiveDateTime,
    pub end_date: NaiveDateTime,
}

impl Event {
    pub fn new(
        id: &str,
        guild_id: i64,
        title: String,
        url: String,
        description: String,
        start_date: NaiveDateTime,
        end_date: NaiveDateTime,
    ) -> Self {
        let id = Uuid::parse_str(id).expect("Bad UUID");
        Event {
            id,
            guild_id,
            title,
            url,
            description,
            start_date,
            end_date,
        }
    }

    pub fn to_event(event_map: HashMap<String, Value>) -> Event {
        let text = |key: &str| {
            event_map
                .get(key)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        };

        Event {
            id: Uuid::parse_str(&text("id")).expect("Bad UUID"),
            guild_id: event_map
                .get("guild_id")
                .and_then(|v| v.as_i64())
                .unwrap_or(0),
            title: text("title"),
            url: text("url"),
            description: text("description"),
            start_date: parse_api_datetime(&text("start_date")).unwrap_or_default(),
            end_date: parse_api_datetime(&text("end_date")).unwrap_or_default(),
        }
    }
}

#[derive(Serialize, Debug)]
pub struct NewEvent {
    pub guild_id: i64,
    pub title: String,
    pub url: String,
    pub description: String,
    pub start_date: NaiveDateTime,
    pub end_date: NaiveDateTime,
}

impl NewEvent {
    pub fn new(
        guild_id: i64,
        title: String,
        url: String,
        description: String,
        start_date: NaiveDateTime,
        end_date: NaiveDateTime,
    ) -> Self {
        NewEvent {
            guild_id,
            title,
            url,
            description,
            start_date,
            end_date,
        }
    }
}

fn parse_api_datetime(s: &str) -> Option<NaiveDateTime> {
    NaiveDateTime::parse_from_str(s, API_FMT).ok()
}

/// Parses a date a person typed.
///
/// Returns `None` rather than panicking. The previous version called `.expect()`
/// on user input, so a mistyped date took the command down instead of saying
/// what was wrong.
fn parse_input_datetime(s: &str) -> Option<NaiveDateTime> {
    NaiveDateTime::parse_from_str(s.trim(), INPUT_FMT).ok()
}

fn date_help() -> String {
    format!(
        "Dates must look like `08/31/2026 5:00PM`, in Pacific time (currently {}).",
        pacific::abbrev_at(pacific_now_naive())
    )
}

/// What a Pacific wall clock reads right now.
fn pacific_now_naive() -> NaiveDateTime {
    chrono::Utc::now()
        .with_timezone(&pacific::PACIFIC)
        .naive_local()
}

fn event_embed(title: &str, fields: Vec<(String, String, bool)>) -> CreateEmbed {
    let mut embed = CreateEmbed::default();
    embed
        .title(title)
        .image("attachment://cork-board.png")
        .fields(fields)
        .timestamp(Timestamp::now());
    embed
}

fn describe(event: &Event) -> String {
    // The abbreviation is resolved per event date, not from today: an event in
    // December is PST even when you are looking at it in July.
    format!(
        "[{}]({}): {}\n**Start:** {} {}\n**End:** {} {}",
        event.title,
        event.url,
        event.description,
        event.start_date.format(INPUT_FMT),
        pacific::abbrev_at(event.start_date),
        event.end_date.format(INPUT_FMT),
        pacific::abbrev_at(event.end_date)
    )
}

/// Reads both user-typed dates, reporting whichever one is malformed.
async fn read_dates(
    inv: &Invocation<'_>,
) -> Option<(NaiveDateTime, NaiveDateTime)> {
    let start_raw = inv.string("start").unwrap_or_default();
    let end_raw = inv.string("end").unwrap_or_default();

    let start = match parse_input_datetime(&start_raw) {
        Some(d) => d,
        None => {
            let _ = inv
                .fail_now(
                    format!("Could not read the start date `{}`. {}", start_raw, date_help()),
                    false,
                )
                .await;
            return None;
        }
    };

    let end = match parse_input_datetime(&end_raw) {
        Some(d) => d,
        None => {
            let _ = inv
                .fail_now(
                    format!("Could not read the end date `{}`. {}", end_raw, date_help()),
                    false,
                )
                .await;
            return None;
        }
    };

    if end < start {
        let _ = inv
            .fail_now("The end date is before the start date.", false)
            .await;
        return None;
    }

    Some((start, end))
}

pub async fn slash_events(inv: &Invocation<'_>) -> serenity::Result<()> {
    let guild_id = match require_guild(inv).await {
        Some(g) => g,
        None => return Ok(()),
    };

    let events: Vec<Event> = match reqwest::get(format!(
        "http://localhost:8000/api/v1/event/current/guild/{}",
        guild_id
    ))
    .await
    {
        Ok(r) => match r.json::<Vec<HashMap<String, Value>>>().await {
            Ok(list) => list.into_iter().map(Event::to_event).collect(),
            Err(_) => return inv.fail_now("Could not read the events.", false).await,
        },
        Err(_) => return inv.fail_now("Could not reach the server.", false).await,
    };

    let mut fields: Vec<(String, String, bool)> = Vec::new();
    for (i, event) in events.iter().enumerate() {
        fields.push((format!("{}.", i + 1), describe(event), false));
    }

    if fields.is_empty() {
        fields.push((
            "Empty!".to_string(),
            "No current events found!".to_string(),
            false,
        ));
    }

    inv.reply_with_file(
        event_embed("Events (Pacific time)", fields),
        BOARD_IMAGE,
        false,
    )
    .await
}

pub async fn slash_add_event(inv: &Invocation<'_>) -> serenity::Result<()> {
    if !validation::has_corkboard_role(inv).await {
        return Ok(());
    }

    let guild_id = match require_guild(inv).await {
        Some(g) => i64::from(g),
        None => return Ok(()),
    };

    let (start, end) = match read_dates(inv).await {
        Some(d) => d,
        None => return Ok(()),
    };

    let new = NewEvent::new(
        guild_id,
        inv.string("title").unwrap_or_default(),
        inv.string("url").unwrap_or_default(),
        inv.string("description").unwrap_or_default(),
        start,
        end,
    );

    println!("Sending new Event creation request with {:?}", new);
    let client = reqwest::Client::new();
    let body = match client
        .post("http://localhost:8000/api/v1/event")
        .json(&new)
        .send()
        .await
    {
        Ok(r) => match r.json::<HashMap<String, Value>>().await {
            Ok(b) => b,
            Err(_) => return inv.fail_now("Could not read the new event.", false).await,
        },
        Err(_) => return inv.fail_now("Could not reach the server.", false).await,
    };

    let created = Event::to_event(body);

    inv.reply_with_file(
        event_embed(
            "Created New Event (Pacific time)",
            vec![("1. ".to_string(), describe(&created), false)],
        ),
        BOARD_IMAGE,
        false,
    )
    .await
}

pub async fn slash_edit_event(inv: &Invocation<'_>) -> serenity::Result<()> {
    if !validation::has_corkboard_role(inv).await {
        return Ok(());
    }

    let guild_id = match require_guild(inv).await {
        Some(g) => i64::from(g),
        None => return Ok(()),
    };

    let display_id = inv.integer("id").unwrap_or(0) as i32;

    let real_id = match resolve_event_id(guild_id, display_id).await {
        Some(id) => id,
        None => {
            return inv
                .fail_now(
                    "Invalid ID! Run `/events` to see a list of usable IDs.",
                    false,
                )
                .await
        }
    };

    let (start, end) = match read_dates(inv).await {
        Some(d) => d,
        None => return Ok(()),
    };

    let new = Event::new(
        real_id.as_str(),
        guild_id,
        inv.string("title").unwrap_or_default(),
        inv.string("url").unwrap_or_default(),
        inv.string("description").unwrap_or_default(),
        start,
        end,
    );

    println!("Sending Event edit request with {:?}", new);
    let client = reqwest::Client::new();
    let body = match client
        .put(format!("http://localhost:8000/api/v1/event/{}", real_id).as_str())
        .json(&new)
        .send()
        .await
    {
        Ok(r) => match r.json::<HashMap<String, Value>>().await {
            Ok(b) => b,
            Err(_) => return inv.fail_now("Could not read the edited event.", false).await,
        },
        Err(_) => return inv.fail_now("Could not reach the server.", false).await,
    };

    let edited = Event::to_event(body);

    inv.reply_with_file(
        event_embed(
            "Edited Event (Pacific time)",
            vec![(format!("{}. ", display_id), describe(&edited), false)],
        ),
        BOARD_IMAGE,
        false,
    )
    .await
}

pub async fn slash_delete_event(inv: &Invocation<'_>) -> serenity::Result<()> {
    if !validation::has_corkboard_role(inv).await {
        return Ok(());
    }

    let guild_id = match require_guild(inv).await {
        Some(g) => i64::from(g),
        None => return Ok(()),
    };

    let display_id = inv.integer("id").unwrap_or(0) as i32;

    let real_id = match resolve_event_id(guild_id, display_id).await {
        Some(id) => id,
        None => {
            return inv
                .fail_now(
                    "Invalid ID! Run `/events` to see a list of usable IDs.",
                    false,
                )
                .await
        }
    };

    println!("Sending Event delete request with ID {:?}", real_id);
    let client = reqwest::Client::new();
    let body = match client
        .delete(format!("http://localhost:8000/api/v1/event/delete/{}", real_id).as_str())
        .header(CONTENT_TYPE, "application/json")
        .send()
        .await
    {
        Ok(r) => match r.json::<HashMap<String, Value>>().await {
            Ok(b) => b,
            Err(_) => return inv.fail_now("Could not read the deleted event.", false).await,
        },
        Err(_) => return inv.fail_now("Could not reach the server.", false).await,
    };

    let deleted = Event::to_event(body);

    inv.reply_with_file(
        event_embed(
            "Deleted Event (Pacific time)",
            vec![(format!("{}. ", display_id), describe(&deleted), false)],
        ),
        BOARD_IMAGE,
        false,
    )
    .await
}

/// Turns the 1-based number shown by `/events` into the record's real uuid.
async fn resolve_event_id(guild_id: i64, display_id: i32) -> Option<String> {
    retrieve_events_id_map(guild_id)
        .await
        .get(&display_id)
        .cloned()
}

async fn retrieve_events_id_map(guild_id: i64) -> HashMap<i32, String> {
    let mut id_map: HashMap<i32, String> = HashMap::new();

    let resp = match reqwest::get(format!(
        "http://localhost:8000/api/v1/event/current/guild/{}",
        guild_id
    ))
    .await
    {
        Ok(r) => r,
        Err(_) => return id_map,
    };

    let list = match resp.json::<Vec<HashMap<String, Value>>>().await {
        Ok(l) => l,
        Err(_) => return id_map,
    };

    for (i, event_map) in list.into_iter().enumerate() {
        let event = Event::to_event(event_map);
        id_map.insert((i + 1) as i32, event.id.to_string());
    }

    id_map
}
