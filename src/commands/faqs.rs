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
pub struct Faq {
    pub id: Uuid,
    pub guild_id: i64,
    pub question: String,
    pub answer: String,
}

impl Faq {
    pub fn new(id: &str, guild_id: i64, question: String, answer: String) -> Self {
        let id = Uuid::parse_str(id).expect("Bad UUID");
        Faq {
            id,
            guild_id,
            question,
            answer,
        }
    }

    pub fn to_faq(faq_map: HashMap<String, Value>) -> Self {
        Faq::new(
            faq_map.get("id").unwrap().as_str().unwrap(),
            faq_map.get("guild_id").unwrap().as_i64().unwrap(),
            faq_map
                .get("question")
                .unwrap()
                .as_str()
                .unwrap()
                .to_string(),
            faq_map.get("answer").unwrap().as_str().unwrap().to_string(),
        )
    }
}

#[derive(Serialize, Debug)]
pub struct NewFaq {
    pub guild_id: i64,
    pub question: String,
    pub answer: String,
}

impl NewFaq {
    pub fn new(guild_id: i64, question: String, answer: String) -> Self {
        NewFaq {
            guild_id,
            question,
            answer,
        }
    }
}

fn faq_embed(title: &str, fields: Vec<(String, String, bool)>) -> CreateEmbed {
    let mut embed = CreateEmbed::default();
    embed
        .title(title)
        .image("attachment://cork-board.png")
        .fields(fields)
        .timestamp(Timestamp::now());
    embed
}

fn record_field(label: String, body: &HashMap<String, Value>) -> (String, String, bool) {
    let question = body.get("question").and_then(|v| v.as_str()).unwrap_or("?");
    let answer = body.get("answer").and_then(|v| v.as_str()).unwrap_or("");
    (format!("{}{}", label, question), answer.to_string(), false)
}

pub async fn slash_faqs(inv: &Invocation<'_>) -> serenity::Result<()> {
    let guild_id = match require_guild(inv).await {
        Some(g) => g,
        None => return Ok(()),
    };

    let faqs: Vec<Faq> = match reqwest::get(format!(
        "http://localhost:8000/api/v1/faq/guild/{}",
        guild_id
    ))
    .await
    {
        Ok(r) => match r.json::<Vec<HashMap<String, Value>>>().await {
            Ok(list) => list.into_iter().map(Faq::to_faq).collect(),
            Err(_) => return inv.fail_now("Could not read the FAQs.", false).await,
        },
        Err(_) => return inv.fail_now("Could not reach the server.", false).await,
    };

    let mut fields: Vec<(String, String, bool)> = Vec::new();
    if faqs.is_empty() {
        fields.push((
            "FAQs: ".to_string(),
            "No current FAQs found!".to_string(),
            false,
        ));
    } else {
        for (i, faq) in faqs.iter().enumerate() {
            fields.push((
                format!("{}. {}", i + 1, faq.question),
                faq.answer.clone(),
                false,
            ));
        }
    }

    inv.reply_with_file(faq_embed("FAQs", fields), BOARD_IMAGE, false)
        .await
}

pub async fn slash_add_faq(inv: &Invocation<'_>) -> serenity::Result<()> {
    if !validation::has_corkboard_role(inv).await {
        return Ok(());
    }

    let guild_id = match require_guild(inv).await {
        Some(g) => i64::from(g),
        None => return Ok(()),
    };

    let new = NewFaq::new(
        guild_id,
        inv.string("question").unwrap_or_default(),
        inv.string("answer").unwrap_or_default(),
    );

    println!("Sending new FAQ creation request with {:?}", new);
    let client = reqwest::Client::new();
    let body = match client
        .post("http://localhost:8000/api/v1/faq")
        .json(&new)
        .send()
        .await
    {
        Ok(r) => match r.json::<HashMap<String, Value>>().await {
            Ok(b) => b,
            Err(_) => return inv.fail_now("Could not read the new FAQ.", false).await,
        },
        Err(_) => return inv.fail_now("Could not reach the server.", false).await,
    };

    inv.reply_with_file(
        faq_embed(
            "Created New FAQ",
            vec![record_field("1. ".to_string(), &body)],
        ),
        BOARD_IMAGE,
        false,
    )
    .await
}

pub async fn slash_edit_faq(inv: &Invocation<'_>) -> serenity::Result<()> {
    if !validation::has_corkboard_role(inv).await {
        return Ok(());
    }

    let guild_id = match require_guild(inv).await {
        Some(g) => i64::from(g),
        None => return Ok(()),
    };

    let display_id = inv.integer("id").unwrap_or(0) as i32;

    let real_id = match resolve_faq_id(guild_id, display_id).await {
        Some(id) => id,
        None => {
            return inv
                .fail_now("Invalid ID! Run `/faqs` to see a list of usable IDs.", false)
                .await
        }
    };

    let new = Faq::new(
        real_id.as_str(),
        guild_id,
        inv.string("question").unwrap_or_default(),
        inv.string("answer").unwrap_or_default(),
    );

    println!("Sending FAQ edit request with {:?}", new);
    let client = reqwest::Client::new();
    let body = match client
        .put(format!("http://localhost:8000/api/v1/faq/{}", real_id).as_str())
        .json(&new)
        .send()
        .await
    {
        Ok(r) => match r.json::<HashMap<String, Value>>().await {
            Ok(b) => b,
            Err(_) => return inv.fail_now("Could not read the edited FAQ.", false).await,
        },
        Err(_) => return inv.fail_now("Could not reach the server.", false).await,
    };

    inv.reply_with_file(
        faq_embed(
            "Edited FAQ",
            vec![record_field(format!("{}. ", display_id), &body)],
        ),
        BOARD_IMAGE,
        false,
    )
    .await
}

pub async fn slash_delete_faq(inv: &Invocation<'_>) -> serenity::Result<()> {
    if !validation::has_corkboard_role(inv).await {
        return Ok(());
    }

    let guild_id = match require_guild(inv).await {
        Some(g) => i64::from(g),
        None => return Ok(()),
    };

    let display_id = inv.integer("id").unwrap_or(0) as i32;

    let real_id = match resolve_faq_id(guild_id, display_id).await {
        Some(id) => id,
        None => {
            return inv
                .fail_now("Invalid ID! Run `/faqs` to see a list of usable IDs.", false)
                .await
        }
    };

    println!("Sending FAQ delete request with ID {:?}", real_id);
    let client = reqwest::Client::new();
    let body = match client
        .delete(format!("http://localhost:8000/api/v1/faq/delete/{}", real_id).as_str())
        .header(CONTENT_TYPE, "application/json")
        .send()
        .await
    {
        Ok(r) => match r.json::<HashMap<String, Value>>().await {
            Ok(b) => b,
            Err(_) => return inv.fail_now("Could not read the deleted FAQ.", false).await,
        },
        Err(_) => return inv.fail_now("Could not reach the server.", false).await,
    };

    inv.reply_with_file(
        faq_embed(
            "Deleted FAQ",
            vec![record_field(format!("{}. ", display_id), &body)],
        ),
        BOARD_IMAGE,
        false,
    )
    .await
}

/// Turns the 1-based number shown by `/faqs` into the record's real uuid.
async fn resolve_faq_id(guild_id: i64, display_id: i32) -> Option<String> {
    retrieve_faqs_id_map(guild_id)
        .await
        .get(&display_id)
        .cloned()
}

async fn retrieve_faqs_id_map(guild_id: i64) -> HashMap<i32, String> {
    let mut id_map: HashMap<i32, String> = HashMap::new();

    let resp = match reqwest::get(format!(
        "http://localhost:8000/api/v1/faq/guild/{}",
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

    for (i, faq_map) in list.into_iter().enumerate() {
        let faq = Faq::to_faq(faq_map);
        id_map.insert((i + 1) as i32, faq.id.to_string());
    }

    id_map
}
