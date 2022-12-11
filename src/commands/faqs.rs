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
pub struct Faq {
    pub id: Uuid,
    pub last_modified_date: NaiveDateTime,
    pub question: String,
    pub answer: String
}

impl Faq {
    pub fn new(
        id: &str,
        last_modified_date: &str,
        question: String,
        answer: String
    ) -> Self {
        let id = Uuid::parse_str(id).expect("Bad UUID");

        let fmt = "%Y-%m-%dT%H:%M:%S%.f";
        let last_modified_date = NaiveDateTime::parse_from_str(last_modified_date, fmt)
            .expect("Unable to parse NaiveDateTime for Faq.");

        Faq {id, last_modified_date, question, answer}
    }

    pub fn to_faq(faq_map: HashMap<String, String>) -> Self {
        Faq::new(
            faq_map.get("id").unwrap(),
            faq_map.get("last_modified_date").unwrap(),
            faq_map.get("question").unwrap().to_string(),
            faq_map.get("answer").unwrap().to_string()
        )
    }
}

#[command]
#[description = "Retrieves all FAQs."]
async fn faqs(ctx: &Context, msg: &Message) -> CommandResult {
    println!("Got FAQs command..");
    let resp = reqwest::get("http://localhost:8000/api/v1/faq")
        .await?
        .json::<Vec<HashMap<String, String>>>()
        .await?;
    let mut faqs: Vec<Faq> = Vec::new();
    for faq_map in resp {
        faqs.push(Faq::to_faq(faq_map));
    }

    let mut faq_fields: Vec<(String, String, bool)> = Vec::new();
    for faq in faqs {
        faq_fields.push((faq.question, faq.answer, false));
    }

    let _msg = msg
        .channel_id
        .send_message(&ctx.http, |m| {
            m.embed(|e| {
                e.title("FAQs")
                    .image("attachment://cork-board.png")
                    .fields(faq_fields)
                    .timestamp(Timestamp::now())
            })
            .add_file("./resources/cork-board.png")
        })
        .await;

    println!("Finished processing FAQs command!");
    Ok(())
}

