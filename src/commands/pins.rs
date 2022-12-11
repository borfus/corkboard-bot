extern crate serde;
extern crate serde_json;

use std::collections::HashMap;

use serenity::prelude::*;
use serenity::model::channel::Message;
use serenity::framework::standard::macros::command;
use serenity::framework::standard::CommandResult;
use serenity::model::Timestamp;
use serde::{Serialize, Deserialize};
use uuid::Uuid;
use chrono::NaiveDateTime;

#[derive(Serialize, Deserialize, Debug)]
pub struct Pin {
    pub id: Uuid,
    pub last_modified_date: NaiveDateTime,
    pub description: String,
    pub title: String,
    pub url: String
}

impl Pin {
    pub fn new(
        id: &str,
        last_modified_date: &str,
        description: String,
        title: String,
        url: String
    ) -> Self {
        let id = Uuid::parse_str(id).expect("Bad UUID");

        let fmt = "%Y-%m-%dT%H:%M:%S%.f";
        let last_modified_date = NaiveDateTime::parse_from_str(last_modified_date, fmt)
            .expect("Unable to parse NaiveDateTime for Pin.");

        Pin {id, last_modified_date, description, title, url}
    }

    pub fn to_pin(pin_map: HashMap<String, String>) -> Self {
        Pin::new(
            pin_map.get("id").unwrap(),
            pin_map.get("last_modified_date").unwrap(),
            pin_map.get("description").unwrap().to_string(),
            pin_map.get("title").unwrap().to_string(),
            pin_map.get("url").unwrap().to_string()
        )
    }
}

#[command]
#[description = "Retrieves all pins."]
async fn pins(ctx: &Context, msg: &Message) -> CommandResult {
    println!("Got pins command..");
    let resp = reqwest::get("http://localhost:8000/api/v1/pin")
        .await?
        .json::<Vec<HashMap<String, String>>>()
        .await?;
    let mut pins: Vec<Pin> = Vec::new();
    for pin_map in resp {
        pins.push(Pin::to_pin(pin_map));
    }

    let mut pin_fields: Vec<(String, String, bool)> = Vec::new();
    let mut i = 1;
    for pin in pins {
        pin_fields.push((format!("{}.", i), format!("[{}]({}): {}", pin.title, pin.url, pin.description), false));
        i += 1;
    }

    let _msg = msg
        .channel_id
        .send_message(&ctx.http, |m| {
            m.embed(|e| {
                e.title("Pins")
                    .image("attachment://cork-board.png")
                    .fields(pin_fields)
                    .timestamp(Timestamp::now())
            })
            .add_file("./resources/cork-board.png")
        })
        .await;

    println!("Finished processing pins command!");
    Ok(())
}

