use std::collections::HashMap;

use serde::{Serialize, Deserialize};
use serenity::prelude::*;
use serenity::model::channel::Message;
use serenity::framework::standard::macros::command;
use serenity::framework::standard::CommandResult;

#[derive(Serialize, Deserialize, Debug)]
struct Pin {
    id: String,
    last_modified_date: String,
    description: String,
    title: String,
    url: String
}

#[command]
#[description = "Retrieves all pins."]
async fn pins(ctx: &Context, msg: &Message) -> CommandResult {
    println!("Got pins command");
    let resp = reqwest::get("http://localhost:8000/api/v1/pin")
        .await?
        .json::<Vec<HashMap<String, String>>>()
        .await?;
    println!("{:#?}", resp);
    msg.reply(ctx, format!("{:#?}", resp)).await?;
    Ok(())
}

