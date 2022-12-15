extern crate serde;
extern crate serde_json;

use std::collections::HashMap;
use std::error::Error;

use serenity::prelude::*;
use serenity::model::channel::Message;
use serenity::framework::standard::macros::command;
use serenity::framework::standard::CommandResult;
use serenity::model::Timestamp;
use serde_json::Value;

use crate::commands::pins::Pin;
use crate::commands::events::Event;
use crate::commands::faqs::Faq;

#[command]
#[description = "Retrieves all events, pins, and faqs."]
async fn list(ctx: &Context, msg: &Message) -> CommandResult {
    println!("Got list command..");

    let mut all_fields = Vec::new();
    all_fields.push(get_events(msg).await?);
    println!("Got events..");
    all_fields.push(get_pins(msg).await?);
    println!("Got pins..");
    all_fields.push(get_faqs(msg).await?);
    println!("Got faqs..");

    let _msg = msg
        .channel_id
        .send_message(&ctx.http, |m| {
            m.embed(|e| {
                e.title("List Results")
                    .image("attachment://cork-board.png")
                    .fields(all_fields)
                    .timestamp(Timestamp::now())
            })
            .add_file("./resources/cork-board.png")
        })
        .await;

    println!("Finished processing list command!");
    Ok(())
}

async fn get_pins(msg: &Message) -> Result<(String, String, bool), Box<dyn Error + Send + Sync>> {
    let resp = reqwest::get(format!("http://localhost:8000/api/v1/pin/guild/{}", msg.guild_id.unwrap()))
        .await?
        .json::<Vec<HashMap<String, Value>>>()
        .await?;
    let mut pins: Vec<Pin> = Vec::new();
    for pin_map in resp {
        pins.push(Pin::to_pin(pin_map));
    }

    if pins.len() == 0 {
        return Ok(("Pins: ".to_string(), "No current pins found!".to_string(), false));
    }

    let mut pin_descriptions = String::new();
    for pin in pins {
        pin_descriptions.push_str(format!("[{}]({}): {}\n", pin.title, pin.url, pin.description).as_str())
    }

    Ok(("Pins:".to_string(), pin_descriptions, false))
}

async fn get_events(msg: &Message) -> Result<(String, String, bool), Box<dyn Error + Send + Sync>> {
    let resp = reqwest::get(format!("http://localhost:8000/api/v1/event/current/guild/{}", msg.guild_id.unwrap()))
        .await?
        .json::<Vec<HashMap<String, Value>>>()
        .await?;

    let mut events: Vec<Event> = Vec::new();
    for event_map in resp {
        events.push(Event::to_event(event_map));
    }

    if events.len() == 0 {
        return Ok(("Events: ".to_string(), "No current events found!".to_string(), false));
    }

    let mut event_descriptions = String::new();
    for event in events {
        event_descriptions.push_str(format!("[{}]({}): {}\n", event.title, event.url, event.description).as_str());
    }

    Ok(("Events:".to_string(), event_descriptions, false))
}

async fn get_faqs(msg: &Message) -> Result<(String, String, bool), Box<dyn Error + Send + Sync>> {
    let resp = reqwest::get(format!("http://localhost:8000/api/v1/faq/guild/{}", msg.guild_id.unwrap()))
        .await?
        .json::<Vec<HashMap<String, Value>>>()
        .await?;
    let mut faqs: Vec<Faq> = Vec::new();
    for faq_map in resp {
        faqs.push(Faq::to_faq(faq_map));
    }

    if faqs.len() == 0 {
        return Ok(("FAQs: ".to_string(), "No current FAQs found!".to_string(), false));
    }

    let mut faq_strings = String::new();
    for faq in faqs {
        faq_strings.push_str(format!("**{}**\n{}\n\n", faq.question, faq.answer).as_str());
    }

    Ok(("FAQs:".to_string(), faq_strings, false))
}

