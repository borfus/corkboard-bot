extern crate serde;
extern crate serde_json;

use std::collections::HashMap;

use serenity::prelude::*;
use serenity::model::channel::Message;
use serenity::framework::standard::macros::command;
use serenity::framework::standard::{CommandResult, Args};
use serenity::model::Timestamp;
use serde::{Serialize, Deserialize};
use serde_json::Value;
use uuid::Uuid;

use crate::validation::validation;

#[derive(Serialize, Deserialize, Debug)]
pub struct Faq {
    pub id: Uuid,
    pub guild_id: i64,
    pub question: String,
    pub answer: String
}

impl Faq {
    pub fn new(
        id: &str,
        guild_id: i64,
        question: String,
        answer: String
    ) -> Self {
        let id = Uuid::parse_str(id).expect("Bad UUID");
        Faq {id, guild_id, question, answer}
    }

    pub fn to_faq(faq_map: HashMap<String, Value>) -> Self {
        Faq::new(
            faq_map.get("id").unwrap().as_str().unwrap(),
            faq_map.get("guild_id").unwrap().as_i64().unwrap(),
            faq_map.get("question").unwrap().as_str().unwrap().to_string(),
            faq_map.get("answer").unwrap().as_str().unwrap().to_string()
        )
    }
}

#[derive(Serialize, Debug)]
pub struct NewFaq {
    pub guild_id: i64,
    pub question: String,
    pub answer: String
}

impl NewFaq {
    pub fn new(
        guild_id: i64,
        question: String,
        answer: String
    ) -> Self {
        NewFaq {guild_id, question, answer}
    }
}

#[command]
#[description = "Retrieves all FAQs."]
async fn faqs(ctx: &Context, msg: &Message) -> CommandResult {
    println!("Got FAQs command..");
    let resp = reqwest::get(format!("http://localhost:8000/api/v1/faq/guild/{}", msg.guild_id.unwrap()))
        .await?
        // .json::<Vec<HashMap<String, String>>>()
        .json::<Vec<HashMap<String, Value>>>()
        .await?;
    println!("We are here now");
    let mut faqs: Vec<Faq> = Vec::new();
    for faq_map in resp {
        faqs.push(Faq::to_faq(faq_map));
    }

    let mut faq_fields: Vec<(String, String, bool)> = Vec::new();
    let mut i = 1;
    if faqs.len() == 0 {
        faq_fields.push(("FAQs: ".to_string(), "No current FAQs found!".to_string(), false));
    } else {
        for faq in faqs {
            faq_fields.push((format!("{}. {}", i, faq.question), faq.answer, false));
            i += 1;
        }
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
async fn add_faq(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let arg_names = vec!("Question", "Answer");
    if !validation::has_corkboard_role(ctx, msg).await
        || !validation::has_correct_arg_count(ctx, msg, 2, args.len(), arg_names, "add_faq").await {
        return Ok(());
    }

    let guild_id = i64::from(msg.guild_id.unwrap());
    let question = args.single_quoted::<String>().unwrap();
    let answer = args.single_quoted::<String>().unwrap();
    let new = NewFaq::new(guild_id, question, answer);

    println!("Sending new FAQ creation request with {:?}", new);
    let client = reqwest::Client::new();
    let resp = client.post("http://localhost:8000/api/v1/faq")
        .json(&new)
        .send()
        .await?
        .json::<HashMap<String, Value>>()
        .await?;

    let question = resp.get("question").unwrap();
    let answer = resp.get("answer").unwrap();
    let _msg = msg
        .channel_id
        .send_message(&ctx.http, |m| {
            m.embed(|e| {
                e.title("Created New FAQ")
                    .image("attachment://cork-board.png")
                    .field(question.to_string(), answer.to_string(), false)
                    .timestamp(Timestamp::now())
            })
            .add_file("./resources/cork-board.png")
        })
        .await;

    Ok(())
}

#[command]
#[allowed_roles("corkboard")]
#[description = "Edit an existing FAQ."]
#[usage = "FAQ_id \"Question\" \"Answer\""]
async fn edit_faq(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let arg_names = vec!("FAQ_id", "Question", "Answer");
    if !validation::has_corkboard_role(ctx, msg).await 
        || !validation::has_correct_arg_count(ctx, msg, 3, args.len(), arg_names, "edit_faq").await {
        return Ok(());
    } 

    let guild_id = i64::from(msg.guild_id.unwrap());
    let id = args.current().unwrap().to_string();
    args.advance();
    let question = args.single_quoted::<String>().unwrap();
    let answer = args.single_quoted::<String>().unwrap();

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

    let id_map = retrieve_faqs_id_map(guild_id).await;
    let real_id_maybe = id_map.get(&id_int).clone();
    let real_id = match real_id_maybe {
        Some(i) => i,
        None => {
            let _msg = msg
                .channel_id.say(
                    &ctx.http,
                    ":bangbang: Error :bangbang: - Invalid ID! Run the `.faqs` command to see a list of usable IDs."
                )
                .await;
            return Ok(());
        }
    };

    let new = Faq::new(real_id.as_str(), guild_id, question, answer);

    println!("Sending FAQ edit request with {:?}", new);
    let client = reqwest::Client::new();
    let resp = client.put(format!("http://localhost:8000/api/v1/faq/{}", real_id).as_str())
        .json(&new)
        .send()
        .await?
        .json::<HashMap<String, Value>>()
        .await?;

    let question = resp.get("question").unwrap();
    let answer = resp.get("answer").unwrap();
    let _msg = msg
        .channel_id
        .send_message(&ctx.http, |m| {
            m.embed(|e| {
                e.title("Edited FAQ")
                    .image("attachment://cork-board.png")
                    .field(question.to_string(), answer.to_string(), false)
                    .timestamp(Timestamp::now())
            })
            .add_file("./resources/cork-board.png")
        })
        .await;

    Ok(())
}

#[command]
#[allowed_roles("corkboard")]
#[description = "Delete a FAQ."]
#[usage = "FAQ_id"]
async fn delete_faq(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let arg_names = vec!("FAQ_id");
    if !validation::has_corkboard_role(ctx, msg).await 
        || !validation::has_correct_arg_count(ctx, msg, 1, args.len(), arg_names, "delete_faq").await {
        return Ok(());
    } 

    let guild_id = i64::from(msg.guild_id.unwrap());
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

    let id_map = retrieve_faqs_id_map(guild_id).await;
    let real_id_maybe = id_map.get(&id_int).clone();
    let real_id = match real_id_maybe {
        Some(i) => i,
        None => {
            let _msg = msg
                .channel_id.say(
                    &ctx.http,
                    ":bangbang: Error :bangbang: - Invalid ID! Run the `.faqs` command to see a list of usable IDs."
                )
                .await;
            return Ok(());
        }
    };

    println!("Sending FAQ delete request with ID {:?}", real_id);
    let client = reqwest::Client::new();
    let resp = client.get(format!("http://localhost:8000/api/v1/faq/delete/{}", real_id).as_str())
        .send()
        .await?
        .json::<HashMap<String, Value>>()
        .await?;

    let question = resp.get("question").unwrap();
    let answer = resp.get("answer").unwrap();
    let _msg = msg
        .channel_id
        .send_message(&ctx.http, |m| {
            m.embed(|e| {
                e.title("Deleted FAQ")
                    .image("attachment://cork-board.png")
                    .field(question.to_string(), answer.to_string(), false)
                    .timestamp(Timestamp::now())
            })
            .add_file("./resources/cork-board.png")
        })
        .await;

    Ok(())
}

async fn retrieve_faqs_id_map(guild_id: i64) -> HashMap<i32, String> {
    let resp = reqwest::get(format!("http://localhost:8000/api/v1/faq/guild/{}", guild_id))
        .await.unwrap()
        .json::<Vec<HashMap<String, Value>>>()
        .await.unwrap();
    let mut faqs: Vec<Faq> = Vec::new();
    for faq_map in resp {
        faqs.push(Faq::to_faq(faq_map));
    }

    let mut result : HashMap<i32, String> = HashMap::new();
    let mut i = 1;
    for faq in faqs {
        result.insert(i, faq.id.to_string());
        i += 1;
    }

    result
}
