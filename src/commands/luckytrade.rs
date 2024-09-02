use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serenity::framework::standard::macros::command;
use serenity::framework::standard::CommandResult;
use serenity::model::channel::Message;
use serenity::prelude::*;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug)]
pub struct LuckymonHistory {
    pub id: Uuid,
    pub user_id: i64,
    pub date_obtained: NaiveDate,
    pub pokemon_id: i64,
    pub shiny: bool,
    pub pokemon_name: String,
    pub traded: bool,
}

impl LuckymonHistory {
    pub fn new(
        id: &str,
        user_id: i64,
        date_obtained: &str,
        pokemon_id: i64,
        shiny: bool,
        pokemon_name: String,
        traded: bool,
    ) -> Self {
        let id = Uuid::parse_str(id).expect("Bad UUID");

        let fmt = "%Y-%m-%d";
        let date_obtained = NaiveDate::parse_from_str(date_obtained, fmt)
            .expect("Unable to parse date_obtained NaiveDate for LuckymonHistory.");

        LuckymonHistory {
            id,
            user_id,
            date_obtained,
            pokemon_id,
            shiny,
            pokemon_name,
            traded,
        }
    }

    pub fn to_hist(hist_map: HashMap<String, Value>) -> Self {
        LuckymonHistory::new(
            hist_map.get("id").unwrap().as_str().unwrap(),
            hist_map.get("user_id").unwrap().as_i64().unwrap(),
            hist_map.get("date_obtained").unwrap().as_str().unwrap(),
            hist_map.get("pokemon_id").unwrap().as_i64().unwrap(),
            hist_map.get("shiny").unwrap().as_bool().unwrap(),
            hist_map
                .get("pokemon_name")
                .unwrap()
                .as_str()
                .unwrap()
                .to_string(),
            hist_map.get("traded").unwrap().as_bool().unwrap(),
        )
    }
}

#[command]
#[description = "Trade Your Luckymon With Other Users."]
async fn luckytrade(ctx: &Context, msg: &Message) -> CommandResult {
    Ok(())
}
