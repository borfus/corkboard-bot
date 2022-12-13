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
use chrono::NaiveDateTime;

use crate::validation::validation;

#[derive(Serialize, Deserialize, Debug)]
pub struct Event {
    pub id: Uuid,
    pub last_modified_date: NaiveDateTime,
    pub description: String,
    pub title: String,
    pub url: String,
    pub start_date: NaiveDateTime,
    pub end_date: NaiveDateTime
}

impl Event {
    pub fn new(
        id: &str,
        last_modified_date: &str,
        description: String,
        title: String,
        url: String,
        start_date: &str,
        end_date: &str 
    ) -> Self {
        let id = Uuid::parse_str(id).expect("Bad UUID");

        let fmt = "%Y-%m-%dT%H:%M:%S%.f";
        let last_modified_date = NaiveDateTime::parse_from_str(last_modified_date, fmt)
            .expect("Unable to parse last_modified_date NaiveDateTime for Event.");

        let start_date = NaiveDateTime::parse_from_str(start_date, fmt)
            .expect("Unable to parse start_date NaiveDateTime for Event.");

        let end_date = NaiveDateTime::parse_from_str(end_date, fmt)
            .expect("Unable to parse end_date NaiveDateTime for Event.");

        Event {id, last_modified_date, description, title, url, start_date, end_date}
    }

    pub fn to_event(event_map: HashMap<String, String>) -> Event {
        Event::new(
            event_map.get("id").unwrap(),
            event_map.get("last_modified_date").unwrap(),
            event_map.get("description").unwrap().to_string(),
            event_map.get("title").unwrap().to_string(),
            event_map.get("url").unwrap().to_string(),
            event_map.get("start_date").unwrap(),
            event_map.get("end_date").unwrap()
        )
    }
}

#[command]
#[description = "Retrieves all events."]
async fn events(ctx: &Context, msg: &Message) -> CommandResult {
    println!("Got events command..");
    let resp = reqwest::get("http://localhost:8000/api/v1/event/current")
        .await?
        .json::<Vec<HashMap<String, String>>>()
        .await?;

    let mut events: Vec<Event> = Vec::new();
    for event_map in resp {
        events.push(Event::to_event(event_map));
    }

    let mut event_fields: Vec<(String, String, bool)> = Vec::new();
    let mut i = 1;
    for event in events {
        event_fields.push((format!("{}.", i), format!("[{}]({}): {}", event.title, event.url, event.description), false));
        i += 1;
    }

    if event_fields.len() == 0 {
        event_fields.push(("Empty!".to_string(), "No current events found!".to_string(), false));
    }

    let _msg = msg
        .channel_id
        .send_message(&ctx.http, |m| {
            m.embed(|e| {
                e.title("Events")
                    .image("attachment://cork-board.png")
                    .fields(event_fields)
                    .timestamp(Timestamp::now())
            })
            .add_file("./resources/cork-board.png")
        })
        .await;

    println!("Finished processing events command!");
    Ok(())
}

#[command]
#[allowed_roles("corkboard")]
#[description = "Add an Event."]
#[usage = "title url description start_date end_date"]
async fn add_event(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    if !validation::has_corkboard_role(ctx, msg).await {
        let _msg = msg
            .channel_id.say(&ctx.http, ":bangbang: Error :bangbang: - Only users with the `corkboard` role can execute this command.")
            .await;
        return Ok(());
    } else if args.len() != 5 {
        let _msg = msg
            .channel_id.say(&ctx.http, ":bangbang: Error :bangbang: - the `add_event` command requires 5 arguments: Title, URL, Description, Start Date, and End Date\n\nSee `.help add_event` for more usage details.")
            .await;
        return Ok(());
    }

    Ok(())
}

#[command]
#[allowed_roles("corkboard")]
#[description = "Edit an Event."]
#[usage = "event_id title url description start_date end_date"]
async fn edit_event(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    if !validation::has_corkboard_role(ctx, msg).await {
        let _msg = msg
            .channel_id.say(&ctx.http, ":bangbang: Error :bangbang: - Only users with the `corkboard` role can execute this command.")
            .await;
        return Ok(());
    } else if args.len() != 6 {
        let _msg = msg
            .channel_id.say(&ctx.http, ":bangbang: Error :bangbang: - the `edit_event` command requires 6 arguments: event_id, Title, URL, Description, Start Date, End Date\n\nSee `.help edit_event` for more usage details.")
            .await;
        return Ok(());
    }

    Ok(())
}

#[command]
#[allowed_roles("corkboard")]
#[description = "Delete an Event."]
#[usage = "event_id"]
async fn delete_event(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    if !validation::has_corkboard_role(ctx, msg).await {
        let _msg = msg
            .channel_id.say(&ctx.http, ":bangbang: Error :bangbang: - Only users with the `corkboard` role can execute this command.")
            .await;
        return Ok(());
    } else if args.len() != 1 {
        let _msg = msg
            .channel_id.say(&ctx.http, ":bangbang: Error :bangbang: - the `delete_event` command requires 1 argument: event_id\n\nSee `.help delete_event` for more usage details.")
            .await;
        return Ok(());
    }

    Ok(())
}

