extern crate serde;
extern crate serde_json;

use std::collections::HashMap;

use reqwest::header::CONTENT_TYPE;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serenity::builder::CreateEmbed;
use serenity::model::Timestamp;
use uuid::Uuid;

use crate::slash::{require_guild, Invocation};
use crate::validation::validation;

const BOARD_IMAGE: &str = "./resources/cork-board.png";

#[derive(Serialize, Deserialize, Debug)]
pub struct Pin {
    pub id: Uuid,
    pub guild_id: i64,
    pub title: String,
    pub url: String,
    pub description: String,
}

impl Pin {
    pub fn new(id: &str, guild_id: i64, title: String, url: String, description: String) -> Self {
        let id = Uuid::parse_str(id).expect("Bad UUID");
        Pin {
            id,
            guild_id,
            title,
            url,
            description,
        }
    }

    pub fn to_pin(pin_map: HashMap<String, Value>) -> Self {
        Pin::new(
            pin_map.get("id").unwrap().as_str().unwrap(),
            pin_map.get("guild_id").unwrap().as_i64().unwrap(),
            pin_map.get("title").unwrap().as_str().unwrap().to_string(),
            pin_map.get("url").unwrap().as_str().unwrap().to_string(),
            pin_map
                .get("description")
                .unwrap()
                .as_str()
                .unwrap()
                .to_string(),
        )
    }
}

#[derive(Serialize, Debug)]
pub struct NewPin {
    pub guild_id: i64,
    pub title: String,
    pub url: String,
    pub description: String,
}

impl NewPin {
    pub fn new(guild_id: i64, title: String, url: String, description: String) -> Self {
        NewPin {
            guild_id,
            title,
            url,
            description,
        }
    }
}

fn pin_embed(title: &str, fields: Vec<(String, String, bool)>) -> CreateEmbed {
    let mut embed = CreateEmbed::default();
    embed
        .title(title)
        .image("attachment://cork-board.png")
        .fields(fields)
        .timestamp(Timestamp::now());
    embed
}

/// Renders one record from an API response body.
fn record_field(label: String, body: &HashMap<String, Value>) -> (String, String, bool) {
    let title = body.get("title").and_then(|v| v.as_str()).unwrap_or("?");
    let url = body.get("url").and_then(|v| v.as_str()).unwrap_or("");
    let description = body
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    (label, format!("[{}]({}): {}", title, url, description), false)
}

pub async fn slash_pins(inv: &Invocation<'_>) -> serenity::Result<()> {
    let guild_id = match require_guild(inv).await {
        Some(g) => g,
        None => return Ok(()),
    };

    let resp = reqwest::get(format!(
        "http://localhost:8000/api/v1/pin/guild/{}",
        guild_id
    ))
    .await;

    let pins: Vec<Pin> = match resp {
        Ok(r) => match r.json::<Vec<HashMap<String, Value>>>().await {
            Ok(list) => list.into_iter().map(Pin::to_pin).collect(),
            Err(_) => return inv.fail_now("Could not read the pins.", false).await,
        },
        Err(_) => return inv.fail_now("Could not reach the server.", false).await,
    };

    let mut fields: Vec<(String, String, bool)> = Vec::new();
    if pins.is_empty() {
        fields.push((
            "Pins: ".to_string(),
            "No current pins found!".to_string(),
            false,
        ));
    } else {
        for (i, pin) in pins.iter().enumerate() {
            fields.push((
                format!("{}.", i + 1),
                format!("[{}]({}): {}", pin.title, pin.url, pin.description),
                false,
            ));
        }
    }

    inv.reply_with_file(pin_embed("Pins", fields), BOARD_IMAGE, false)
        .await
}

pub async fn slash_add_pin(inv: &Invocation<'_>) -> serenity::Result<()> {
    if !validation::has_corkboard_role(inv).await {
        return Ok(());
    }

    let guild_id = match require_guild(inv).await {
        Some(g) => i64::from(g),
        None => return Ok(()),
    };

    let new = NewPin::new(
        guild_id,
        inv.string("title").unwrap_or_default(),
        inv.string("url").unwrap_or_default(),
        inv.string("description").unwrap_or_default(),
    );

    println!("Sending new Pin creation request with {:?}", new);
    let client = reqwest::Client::new();
    let resp = client
        .post("http://localhost:8000/api/v1/pin")
        .json(&new)
        .send()
        .await;

    let body = match resp {
        Ok(r) => match r.json::<HashMap<String, Value>>().await {
            Ok(b) => b,
            Err(_) => return inv.fail_now("Could not read the new pin.", false).await,
        },
        Err(_) => return inv.fail_now("Could not reach the server.", false).await,
    };

    inv.reply_with_file(
        pin_embed("Created New Pin", vec![record_field("1. ".to_string(), &body)]),
        BOARD_IMAGE,
        false,
    )
    .await
}

pub async fn slash_edit_pin(inv: &Invocation<'_>) -> serenity::Result<()> {
    if !validation::has_corkboard_role(inv).await {
        return Ok(());
    }

    let guild_id = match require_guild(inv).await {
        Some(g) => i64::from(g),
        None => return Ok(()),
    };

    let display_id = inv.integer("id").unwrap_or(0) as i32;

    let real_id = match resolve_pin_id(guild_id, display_id).await {
        Some(id) => id,
        None => {
            return inv
                .fail_now(
                    "Invalid ID! Run `/pins` to see a list of usable IDs.",
                    false,
                )
                .await
        }
    };

    let new = Pin::new(
        real_id.as_str(),
        guild_id,
        inv.string("title").unwrap_or_default(),
        inv.string("url").unwrap_or_default(),
        inv.string("description").unwrap_or_default(),
    );

    println!("Sending Pin edit request with {:?}", new);
    let client = reqwest::Client::new();
    let resp = client
        .put(format!("http://localhost:8000/api/v1/pin/{}", real_id).as_str())
        .json(&new)
        .send()
        .await;

    let body = match resp {
        Ok(r) => match r.json::<HashMap<String, Value>>().await {
            Ok(b) => b,
            Err(_) => return inv.fail_now("Could not read the edited pin.", false).await,
        },
        Err(_) => return inv.fail_now("Could not reach the server.", false).await,
    };

    inv.reply_with_file(
        pin_embed(
            "Edited Pin",
            vec![record_field(format!("{}. ", display_id), &body)],
        ),
        BOARD_IMAGE,
        false,
    )
    .await
}

pub async fn slash_delete_pin(inv: &Invocation<'_>) -> serenity::Result<()> {
    if !validation::has_corkboard_role(inv).await {
        return Ok(());
    }

    let guild_id = match require_guild(inv).await {
        Some(g) => i64::from(g),
        None => return Ok(()),
    };

    let display_id = inv.integer("id").unwrap_or(0) as i32;

    let real_id = match resolve_pin_id(guild_id, display_id).await {
        Some(id) => id,
        None => {
            return inv
                .fail_now(
                    "Invalid ID! Run `/pins` to see a list of usable IDs.",
                    false,
                )
                .await
        }
    };

    println!("Sending Pin delete request with ID {:?}", real_id);
    let client = reqwest::Client::new();
    let resp = client
        .delete(format!("http://localhost:8000/api/v1/pin/delete/{}", real_id).as_str())
        .header(CONTENT_TYPE, "application/json")
        .send()
        .await;

    let body = match resp {
        Ok(r) => match r.json::<HashMap<String, Value>>().await {
            Ok(b) => b,
            Err(_) => return inv.fail_now("Could not read the deleted pin.", false).await,
        },
        Err(_) => return inv.fail_now("Could not reach the server.", false).await,
    };

    inv.reply_with_file(
        pin_embed(
            "Deleted Pin",
            vec![record_field(format!("{}. ", display_id), &body)],
        ),
        BOARD_IMAGE,
        false,
    )
    .await
}

/// Turns the 1-based number shown by `/pins` into the record's real uuid.
async fn resolve_pin_id(guild_id: i64, display_id: i32) -> Option<String> {
    retrieve_pins_id_map(guild_id).await.get(&display_id).cloned()
}

async fn retrieve_pins_id_map(guild_id: i64) -> HashMap<i32, String> {
    let mut id_map: HashMap<i32, String> = HashMap::new();

    let resp = match reqwest::get(format!(
        "http://localhost:8000/api/v1/pin/guild/{}",
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

    for (i, pin_map) in list.into_iter().enumerate() {
        let pin = Pin::to_pin(pin_map);
        id_map.insert((i + 1) as i32, pin.id.to_string());
    }

    id_map
}
