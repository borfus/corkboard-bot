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
    pub title: String,
    pub url: String,
    pub description: String,
    pub start_date: NaiveDateTime,
    pub end_date: NaiveDateTime
}

impl Event {
    pub fn new(
        id: &str,
        title: String,
        url: String,
        description: String,
        start_date: &str,
        end_date: &str 
    ) -> Self {
        let id = Uuid::parse_str(id).expect("Bad UUID");

        let fmt = "%Y-%m-%dT%H:%M:%S%.f";
        let start_date = NaiveDateTime::parse_from_str(start_date, fmt)
            .expect("Unable to parse start_date NaiveDateTime for Event.");
        let end_date = NaiveDateTime::parse_from_str(end_date, fmt)
            .expect("Unable to parse end_date NaiveDateTime for Event.");

        Event {id, title, url, description, start_date, end_date}
    }

    pub fn to_event(event_map: HashMap<String, String>) -> Event {
        Event::new(
            event_map.get("id").unwrap(),
            event_map.get("title").unwrap().to_string(),
            event_map.get("url").unwrap().to_string(),
            event_map.get("description").unwrap().to_string(),
            event_map.get("start_date").unwrap(),
            event_map.get("end_date").unwrap()
        )
    }
}

#[derive(Serialize, Debug)]
pub struct NewEvent {
    pub title: String,
    pub url: String,
    pub description: String,
    pub start_date: NaiveDateTime,
    pub end_date: NaiveDateTime
}

impl NewEvent {
    pub fn new(
        title: String,
        url: String,
        description: String,
        start_date: &str,
        end_date: &str
    ) -> Self {
        let fmt = "%m/%d/%Y %-I:%M%p";
        let start_date = NaiveDateTime::parse_from_str(start_date, fmt)
            .expect("Unable to parse start_date NaiveDateTime for Event.");
        let end_date = NaiveDateTime::parse_from_str(end_date, fmt)
            .expect("Unable to parse end_date NaiveDateTime for Event.");

        NewEvent {title, url, description, start_date, end_date}
    }
}

#[command]
#[description = "Retrieves all events. All times using PST/PDT."]
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
        event_fields.push(
            (
                format!("{}.", i),
                format!(
                    "[{}]({}): {}\n**Start:** {}\n**End:** {}",
                    event.title,
                    event.url,
                    event.description,
                    event.start_date.format("%m/%d/%Y %-I:%M%p").to_string(),
                    event.end_date.format("%m/%d/%Y %-I:%M%p").to_string()
                ),
                false
            )
        );
        i += 1;
    }

    if event_fields.len() == 0 {
        event_fields.push(("Empty!".to_string(), "No current events found!".to_string(), false));
    }

    let _msg = msg
        .channel_id
        .send_message(&ctx.http, |m| {
            m.embed(|e| {
                e.title("Events (using PST/PDT)")
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
#[description = "Add an Event. All times using PST/PDT."]
#[usage = "title url description start_date end_date"]
async fn add_event(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let arg_names = vec!("Title", "URL", "Description", "Start Date", "End Date");
    if !validation::has_corkboard_role(ctx, msg).await 
        || !validation::has_correct_arg_count(ctx, msg, 5, args.len(), arg_names, "add_event").await {
        return Ok(());
    }

    let title = args.single_quoted::<String>().unwrap();
    let url = args.single_quoted::<String>().unwrap();
    let description = args.single_quoted::<String>().unwrap();
    let start_date = args.single_quoted::<String>().unwrap();
    let end_date = args.single_quoted::<String>().unwrap();
    let new = NewEvent::new(title, url, description, start_date.as_str(), end_date.as_str());

    println!("Sending new Event creation request with {:?}", new);
    let client = reqwest::Client::new();
    let resp = client.post("http://localhost:8000/api/v1/event")
        .json(&new)
        .send()
        .await?
        .json::<Vec<HashMap<String, String>>>()
        .await?;

    let title = resp.get(0).unwrap().get("title").unwrap();
    let url = resp.get(0).unwrap().get("url").unwrap();
    let description = resp.get(0).unwrap().get("description").unwrap();
    let start_date = resp.get(0).unwrap().get("start_date").unwrap();
    let end_date = resp.get(0).unwrap().get("end_date").unwrap();

    let fmt = "%Y-%m-%dT%H:%M:%S%.f";
    let start_date = NaiveDateTime::parse_from_str(start_date, fmt)
        .expect("Unable to parse start_date NaiveDateTime for Event.");
    let end_date = NaiveDateTime::parse_from_str(end_date, fmt)
        .expect("Unable to parse start_date NaiveDateTime for Event.");

    let _msg = msg
        .channel_id
        .send_message(&ctx.http, |m| {
            m.embed(|e| {
                e.title("Created New Event (using PST/PDT)")
                    .image("attachment://cork-board.png")
                    .field(
                        format!("1. "),
                        format!(
                            "[{}]({}): {}\n**Start:** {}\n**End:** {}",
                            title,
                            url,
                            description,
                            start_date.format("%m/%d/%Y %-I:%M%p").to_string(),
                            end_date.format("%m/%d/%Y %-I:%M%p").to_string()
                        ),
                        false
                    )
                    .timestamp(Timestamp::now())
            })
            .add_file("./resources/cork-board.png")
        })
        .await;

    Ok(())
}

#[command]
#[allowed_roles("corkboard")]
#[description = "Edit an Event. All times using PST/PDT."]
#[usage = "event_id title url description start_date end_date"]
async fn edit_event(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let arg_names = vec!("Event_id", "Title", "URL", "Description", "Start Date", "End Date");
    if !validation::has_corkboard_role(ctx, msg).await 
        || !validation::has_correct_arg_count(ctx, msg, 6, args.len(), arg_names, "edit_event").await {
        return Ok(());
    } 

    let id = args.current().unwrap().to_string();
    args.advance();
    let title = args.single_quoted::<String>().unwrap();
    let url = args.single_quoted::<String>().unwrap();
    let description = args.single_quoted::<String>().unwrap();
    let start_date = args.single_quoted::<String>().unwrap();
    let end_date = args.single_quoted::<String>().unwrap();

    let fmt = "%m/%d/%Y %-I:%M%p";
    let start_date = NaiveDateTime::parse_from_str(start_date.as_str(), fmt)
        .expect("Unable to parse start_date NaiveDateTime for Event.");
    let end_date = NaiveDateTime::parse_from_str(end_date.as_str(), fmt)
        .expect("Unable to parse start_date NaiveDateTime for Event.");

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

    let id_map = retrieve_events_id_map().await;
    let real_id_maybe = id_map.get(&id_int).clone();
    let real_id = match real_id_maybe {
        Some(i) => i,
        None => {
            let _msg = msg
                .channel_id.say(
                    &ctx.http,
                    ":bangbang: Error :bangbang: - Invalid ID! Run the `.events` command to see a list of usable IDs."
                )
                .await;
            return Ok(());
        }
    };

    let new = Event::new(
        real_id.as_str(),
        title,
        url,
        description,
        start_date.format("%Y-%m-%dT%H:%M:%S").to_string().as_str(),
        end_date.format("%Y-%m-%dT%H:%M:%S").to_string().as_str()
    );

    println!("Sending Event edit request with {:?}", new);
    let client = reqwest::Client::new();
    let resp = client.put(format!("http://localhost:8000/api/v1/event/{}", real_id).as_str())
        .json(&new)
        .send()
        .await?
        .json::<HashMap<String, String>>()
        .await?;

    let title = resp.get("title").unwrap();
    let url = resp.get("url").unwrap();
    let description = resp.get("description").unwrap();
    let start_date = resp.get("start_date").unwrap();
    let end_date = resp.get("end_date").unwrap();

    let fmt = "%Y-%m-%dT%H:%M:%S%.f";
    let start_date = NaiveDateTime::parse_from_str(start_date, fmt)
        .expect("Unable to parse start_date NaiveDateTime for Event.");
    let end_date = NaiveDateTime::parse_from_str(end_date, fmt)
        .expect("Unable to parse start_date NaiveDateTime for Event.");

    let _msg = msg
        .channel_id
        .send_message(&ctx.http, |m| {
            m.embed(|e| {
                e.title("Edited Event (using PST/PDT)")
                    .image("attachment://cork-board.png")
                    .field(
                        format!("{}. ", id),
                        format!(
                            "[{}]({}): {}\n**Start:** {}\n**End:** {}",
                            title,
                            url,
                            description,
                            start_date.format("%m/%d/%Y %-I:%M%p").to_string(),
                            end_date.format("%m/%d/%Y %-I:%M%p").to_string()
                        ),
                        false
                    )
                    .timestamp(Timestamp::now())
            })
            .add_file("./resources/cork-board.png")
        })
        .await;

    Ok(())
}

#[command]
#[allowed_roles("corkboard")]
#[description = "Delete an Event. All times using PST/PDT."]
#[usage = "event_id"]
async fn delete_event(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let arg_names = vec!("Event_id");
    if !validation::has_corkboard_role(ctx, msg).await 
        || !validation::has_correct_arg_count(ctx, msg, 1, args.len(), arg_names, "delete_event").await {
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

    let id_map = retrieve_events_id_map().await;
    let real_id_maybe = id_map.get(&id_int).clone();
    let real_id = match real_id_maybe {
        Some(i) => i,
        None => {
            let _msg = msg
                .channel_id.say(
                    &ctx.http,
                    ":bangbang: Error :bangbang: - Invalid ID! Run the `.events` command to see a list of usable IDs."
                )
                .await;
            return Ok(());
        }
    };

    println!("Sending Event delete request with ID {:?}", real_id);
    let client = reqwest::Client::new();
    let resp = client.get(format!("http://localhost:8000/api/v1/event/delete/{}", real_id).as_str())
        .send()
        .await?
        .json::<HashMap<String, String>>()
        .await?;

    let title = resp.get("title").unwrap();
    let url = resp.get("url").unwrap();
    let description = resp.get("description").unwrap();
    let start_date = resp.get("start_date").unwrap();
    let end_date = resp.get("end_date").unwrap();

    let fmt = "%Y-%m-%dT%H:%M:%S%.f";
    let start_date = NaiveDateTime::parse_from_str(start_date, fmt)
        .expect("Unable to parse start_date NaiveDateTime for Event.");
    let end_date = NaiveDateTime::parse_from_str(end_date, fmt)
        .expect("Unable to parse start_date NaiveDateTime for Event.");

    let _msg = msg
        .channel_id
        .send_message(&ctx.http, |m| {
            m.embed(|e| {
                e.title("Deleted Event (using PST/PDT)")
                    .image("attachment://cork-board.png")
                    .field(
                        format!("{}. ", id),
                        format!(
                            "[{}]({}): {}\n**Start:** {}\n**End:** {}",
                            title,
                            url,
                            description,
                            start_date.format("%m/%d/%Y %-I:%M%p").to_string(),
                            end_date.format("%m/%d/%Y %-I:%M%p").to_string()
                        ),
                        false
                    )
                    .timestamp(Timestamp::now())
            })
            .add_file("./resources/cork-board.png")
        })
        .await;

    Ok(())
}

async fn retrieve_events_id_map() -> HashMap<i32, String> {
    let resp = reqwest::get("http://localhost:8000/api/v1/event/current")
        .await.unwrap()
        .json::<Vec<HashMap<String, String>>>()
        .await.unwrap();
    let mut events: Vec<Event> = Vec::new();
    for event_map in resp {
        events.push(Event::to_event(event_map));
    }

    let mut result : HashMap<i32, String> = HashMap::new();
    let mut i = 1;
    for event in events {
        result.insert(i, event.id.to_string());
        i += 1;
    }

    result
}

