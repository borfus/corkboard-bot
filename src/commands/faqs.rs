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
pub struct Faq {
    pub id: Uuid,
    pub last_modified_date: NaiveDateTime,
    pub question: String,
    pub answer: String
}

#[derive(Serialize, Debug)]
pub struct NewFaq {
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

#[command]
#[allowed_roles("corkboard")]
#[description = "Create new FAQ."]
#[usage = "\"Question\" \"Answer\""]
async fn add_faq(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    if !validation::has_corkboard_role(ctx, msg).await {
        let _msg = msg
            .channel_id.say(&ctx.http, ":bangbang: Error :bangbang: - Only users with the `corkboard` role can execute this command.")
            .await;
        return Ok(());
    } else if args.len() != 2 {
        let _msg = msg
            .channel_id.say(&ctx.http, ":bangbang: Error :bangbang: - the `add_faq` command requires 2 arguments: Question and Answer\n\nSee `.help add_faq`  for more usage details.")
            .await;
        return Ok(());
    }

    Ok(())
}

#[command]
#[allowed_roles("corkboard")]
#[description = "Edit an existing FAQ."]
#[usage = "FAQ_id \"Question\" \"Answer\""]
async fn edit_faq(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    if !validation::has_corkboard_role(ctx, msg).await {
        let _msg = msg
            .channel_id.say(&ctx.http, ":bangbang: Error :bangbang: - Only users with the `corkboard` role can execute this command.")
            .await;
        return Ok(());
    } else if args.len() != 2 {
        let _msg = msg
            .channel_id.say(&ctx.http, ":bangbang: Error :bangbang: - the `edit_faq` command requires 3 arguments: FAQ_id, Question, and Answer\n\nSee `.help edit_faq`  for more usage details.")
            .await;
        return Ok(());
    }

    Ok(())
}

#[command]
#[allowed_roles("corkboard")]
#[description = "Delete a FAQ."]
#[usage = "FAQ_id"]
async fn delete_faq(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    if !validation::has_corkboard_role(ctx, msg).await {
        let _msg = msg
            .channel_id.say(&ctx.http, ":bangbang: Error :bangbang: - Only users with the `corkboard` role can execute this command.")
            .await;
        return Ok(());
    } else if args.len() != 1 {
        let _msg = msg
            .channel_id.say(&ctx.http, ":bangbang: Error :bangbang: - the `delete_faq` command requires 1 argument: FAQ_id\n\nSee `.help delete_faq` for more usage details.")
            .await;
        return Ok(());
    }

    Ok(())
}

