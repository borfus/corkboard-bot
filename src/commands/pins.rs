extern crate serde;
extern crate serde_json;

use std::collections::HashMap;

use serenity::prelude::*;
use serenity::model::channel::Message;
use serenity::framework::standard::macros::command;
use serenity::framework::standard::{CommandResult, Args};
use serenity::model::Timestamp;
use serde::{Serialize, Deserialize};
use uuid::Uuid;

use crate::validation::validation;

#[derive(Serialize, Deserialize, Debug)]
pub struct Pin {
    pub id: Uuid,
    pub title: String,
    pub url: String,
    pub description: String
}

impl Pin {
    pub fn new(
        id: &str,
        title: String,
        url: String,
        description: String
    ) -> Self {
        let id = Uuid::parse_str(id).expect("Bad UUID");
        Pin {id, title, url, description}
    }

    pub fn to_pin(pin_map: HashMap<String, String>) -> Self {
        Pin::new(
            pin_map.get("id").unwrap(),
            pin_map.get("title").unwrap().to_string(),
            pin_map.get("url").unwrap().to_string(),
            pin_map.get("description").unwrap().to_string()
        )
    }
}

#[derive(Serialize, Debug)]
pub struct NewPin {
    pub title: String,
    pub url: String,
    pub description: String
}

impl NewPin {
    pub fn new(
        title: String,
        url: String,
        description: String
    ) -> Self {
        NewPin {title, url, description}
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

#[command]
#[allowed_roles("corkboard")]
#[description = "Add a Pin."]
#[usage = "title url description"]
async fn add_pin(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let arg_names = vec!("Title", "URL", "Description");
    if !validation::has_corkboard_role(ctx, msg).await 
        || !validation::has_correct_arg_count(ctx, msg, 3, args.len(), arg_names, "add_pin").await {
        return Ok(());
    } 

    args.quoted();
    let title = args.current().unwrap().to_string();
    args.advance();
    let url = args.current().unwrap().to_string();
    args.advance();
    let description = args.current().unwrap().to_string();
    let new = NewPin::new(title, url, description);

    println!("Sending new Pin creation request with {:?}", new);
    let client = reqwest::Client::new();
    let resp = client.post("http://localhost:8000/api/v1/pin")
        .json(&new)
        .send()
        .await?
        .json::<Vec<HashMap<String, String>>>()
        .await?;

    let title = resp.get(0).unwrap().get("title").unwrap();
    let url = resp.get(0).unwrap().get("url").unwrap();
    let description = resp.get(0).unwrap().get("description").unwrap();
    let _msg = msg
        .channel_id
        .send_message(&ctx.http, |m| {
            m.embed(|e| {
                e.title("Created New Pin")
                    .image("attachment://cork-board.png")
                    .field("1. ", format!("[{}]({}): {}", title, url, description), false)
                    .timestamp(Timestamp::now())
            })
            .add_file("./resources/cork-board.png")
        })
        .await;

    Ok(())
}

#[command]
#[allowed_roles("corkboard")]
#[description = "Edit a Pin."]
#[usage = "pin_id title url description"]
async fn edit_pin(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let arg_names = vec!("Pin_id", "Title", "URL", "Description");
    if !validation::has_corkboard_role(ctx, msg).await 
        || !validation::has_correct_arg_count(ctx, msg, 4, args.len(), arg_names, "edit_pin").await {
        return Ok(());
    } 

    let id = args.current().unwrap().to_string();
    args.advance();
    let title = args.single_quoted::<String>().unwrap().to_string();
    let url = args.single_quoted::<String>().unwrap().to_string();
    let description = args.single_quoted::<String>().unwrap().to_string();

    let id_int = match id.parse::<i32>() {
        Ok(i) => i,
        _error => {
            let _msg = msg
                .channel_id.say(
                    &ctx.http,
                    ":bangbang: Error :bangbang: - Unable to parse ID."
                )
                .await;
            return Ok(());
        }
    };

    let id_map = retrieve_pins_id_map().await;
    let real_id_maybe = id_map.get(&id_int).clone();
    let real_id = match real_id_maybe {
        Some(i) => i,
        None => {
            let _msg = msg
                .channel_id.say(
                    &ctx.http,
                    ":bangbang: Error :bangbang: - Invalid ID! Run the `.pins` command to see a list of usable IDs."
                )
                .await;
            return Ok(());
        }
    };

    let new = Pin::new(real_id.as_str(), title, url, description);

    println!("Sending Pin edit request with {:?}", new);
    let client = reqwest::Client::new();
    let resp = client.put(format!("http://localhost:8000/api/v1/pin/{}", real_id).as_str())
        .json(&new)
        .send()
        .await?
        .json::<HashMap<String, String>>()
        .await?;

    let title = resp.get("title").unwrap();
    let url = resp.get("url").unwrap();
    let description = resp.get("description").unwrap();
    let _msg = msg
        .channel_id
        .send_message(&ctx.http, |m| {
            m.embed(|e| {
                e.title("Edited New Pin")
                    .image("attachment://cork-board.png")
                    .field(format!("{}. ", id), format!("[{}]({}): {}", title, url, description), false)
                    .timestamp(Timestamp::now())
            })
            .add_file("./resources/cork-board.png")
        })
        .await;

    Ok(())
}

#[command]
#[allowed_roles("corkboard")]
#[description = "Add a Pin."]
#[usage = "pin_id"]
async fn delete_pin(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let arg_names = vec!("Pin_id");
    if !validation::has_corkboard_role(ctx, msg).await 
        || !validation::has_correct_arg_count(ctx, msg, 1, args.len(), arg_names, "delete_pin").await {
        return Ok(());
    } 

    args.quoted();
    let id = args.current().unwrap().to_string();
    let id_int = match id.parse::<i32>() {
        Ok(i) => i,
        _error => {
            let _msg = msg
                .channel_id.say(
                    &ctx.http,
                    ":bangbang: Error :bangbang: - Unable to parse ID."
                )
                .await;
            return Ok(());
        }
    };

    let id_map = retrieve_pins_id_map().await;
    let real_id_maybe = id_map.get(&id_int).clone();
    let real_id = match real_id_maybe {
        Some(i) => i,
        None => {
            let _msg = msg
                .channel_id.say(
                    &ctx.http,
                    ":bangbang: Error :bangbang: - Invalid ID! Run the `.pin` command to see a list of usable IDs."
                )
                .await;
            return Ok(());
        }
    };

    println!("Sending Pin delete request with ID {:?}", real_id);
    let client = reqwest::Client::new();
    let resp = client.get(format!("http://localhost:8000/api/v1/pin/delete/{}", real_id).as_str())
        .send()
        .await?
        .json::<HashMap<String, String>>()
        .await?;

    let title = resp.get("title").unwrap();
    let url = resp.get("url").unwrap();
    let description = resp.get("description").unwrap();
    let _msg = msg
        .channel_id
        .send_message(&ctx.http, |m| {
            m.embed(|e| {
                e.title("Edited New Pin")
                    .image("attachment://cork-board.png")
                    .field("1. ", format!("[{}]({}): {}", title, url, description), false)
                    .timestamp(Timestamp::now())
            })
            .add_file("./resources/cork-board.png")
        })
        .await;

    Ok(())
}

async fn retrieve_pins_id_map() -> HashMap<i32, String> {
    let resp = reqwest::get("http://localhost:8000/api/v1/pin")
        .await.unwrap()
        .json::<Vec<HashMap<String, String>>>()
        .await.unwrap();
    let mut pins: Vec<Pin> = Vec::new();
    for pin_map in resp {
        pins.push(Pin::to_pin(pin_map));
    }

    let mut result : HashMap<i32, String> = HashMap::new();
    let mut i = 1;
    for pin in pins {
        result.insert(i, pin.id.to_string());
        i += 1;
    }

    result
}

