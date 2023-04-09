use std::collections::HashMap;
use std::time::Duration;

use serenity::prelude::*;
use serenity::model::channel::Message;
use serenity::framework::standard::macros::command;
use serenity::framework::standard::CommandResult;
use serenity::model::Timestamp;
use serenity::utils::Colour;
use serenity::model::prelude::ReactionType;

use serde::{Serialize, Deserialize};
use serde_json::Value;
use uuid::Uuid;
use chrono::NaiveDate;

#[derive(Serialize, Deserialize, Debug)]
pub struct LuckymonHistory {
    pub id: Uuid,
    pub user_id: i64,
    pub date_obtained: NaiveDate,
    pub pokemon_id: i64,
    pub shiny: bool,
    pub pokemon_name: String
}

impl LuckymonHistory {
    pub fn new(
        id: &str,
        user_id: i64,
        date_obtained: &str,
        pokemon_id: i64,
        shiny: bool,
        pokemon_name: String
    ) -> Self {
        let id = Uuid::parse_str(id).expect("Bad UUID");

        let fmt = "%Y-%m-%d";
        let date_obtained = NaiveDate::parse_from_str(date_obtained, fmt)
            .expect("Unable to parse date_obtained NaiveDate for LuckymonHistory.");

        LuckymonHistory {id, user_id, date_obtained, pokemon_id, shiny, pokemon_name}
    }

    pub fn to_hist(hist_map: HashMap<String, Value>) -> Self {
        LuckymonHistory::new(
            hist_map.get("id").unwrap().as_str().unwrap(),
            hist_map.get("user_id").unwrap().as_i64().unwrap(),
            hist_map.get("date_obtained").unwrap().as_str().unwrap(),
            hist_map.get("pokemon_id").unwrap().as_i64().unwrap(),
            hist_map.get("shiny").unwrap().as_bool().unwrap(),
            hist_map.get("pokemon_name").unwrap().as_str().unwrap().to_string()
        )
    }
}

#[command]
#[description = "Retrieves Luckymon History for a User."]
async fn luckydex(ctx: &Context, msg: &Message) -> CommandResult {
    println!("Got luckydex command..");
    let resp = reqwest::get(format!("http://localhost:8000/api/v1/luckymon-history/user-id/{}", i64::from(msg.author.id)))
        .await?
        .json::<Vec<HashMap<String, Value>>>()
        .await?;
    let mut hists: Vec<LuckymonHistory> = Vec::new();
    for hist_map in resp {
        hists.push(LuckymonHistory::to_hist(hist_map));
    }

    let items_per_page = 20;
    let total_pages = (hists.len() as f64 / items_per_page as f64).ceil() as usize;
    let mut current_page = 0;

    let mut message = create_embed_page(ctx, msg, &hists, items_per_page, current_page).await?;

    let left_arrow = ReactionType::Unicode("⬅️".to_string());
    let right_arrow = ReactionType::Unicode("➡️".to_string());

    message.react(&ctx.http, left_arrow.clone()).await?;
    message.react(&ctx.http, right_arrow.clone()).await?;

    loop {
        if let Some(reaction) = &message
            .await_reaction(&ctx)
            .timeout(Duration::from_secs(20))
            .author_id(msg.author.id)
            .await
        {
            println!("Reached inside reaction!!");
            let emoji = &reaction.as_inner_ref().emoji;

            if emoji == &left_arrow {
                if current_page > 0 && total_pages != 0 {
                    current_page -= 1;
                }
            } else if emoji == &right_arrow && total_pages != 0 {
                if current_page < total_pages - 1 {
                    current_page += 1;
                }
            }

            update_embed_page(ctx, &mut message, &hists, items_per_page, current_page, &msg).await?;

            if let Err(why) = reaction.as_inner_ref().delete(&ctx.http).await {
                println!("Error deleting reaction: {:?}", why);
            }
        } else {
            break;
        }
    }

    println!("Finished processing luckydex command!");
    Ok(())
}

async fn create_embed_page(
    ctx: &Context,
    msg: &Message,
    data: &[LuckymonHistory],
    items_per_page: usize,
    current_page: usize,
) -> serenity::Result<Message> {
    let start_index = current_page * items_per_page;
    let end_index = usize::min(start_index + items_per_page, data.len());

    let current_data = &data[start_index..end_index];

    let mut pokedex_numbers = Vec::new(); 
    let mut pokemon_names = Vec::new(); 
    let mut dates = Vec::new(); 

    for hist in current_data {
        pokedex_numbers.push(hist.pokemon_id);
        dates.push(hist.date_obtained);

        let mut name = hist.pokemon_name.clone();
        if hist.shiny {
            name = format!("✨{}✨", hist.pokemon_name.clone())
        }
        pokemon_names.push(name);
    }
    let mut total_pages = (data.len() as f64 / items_per_page as f64).ceil() as usize;
    if total_pages == 0 {
        total_pages = 1;
    }

    msg.channel_id
        .send_message(&ctx.http, |m| {
            m.embed(|e| {
                e.title("Luckymon History")
                    .color(Colour::from_rgb(0, 255, 255))
                    .footer(|f| {
                        f.text(
                            format!(
                                "{}: Page {} of {}",
                                &msg.author.name,
                                current_page + 1,
                                total_pages
                            )
                        );
                        f.icon_url(&msg.author.avatar_url().unwrap())
                    });

                e.field(
                    "Pokédex #",
                    pokedex_numbers.iter().map(|s| s.to_string()).collect::<Vec<String>>().join("\n"),
                    true,
                );

                e.field(
                    "Pokémon's Name",
                    pokemon_names.iter().map(|s| s.to_string()).collect::<Vec<String>>().join("\n"),
                    true,
                );

                e.field(
                    "Obtained (YYYY-MM-DD)",
                    dates.iter().map(|s| s.to_string()).collect::<Vec<String>>().join("\n"),
                    true,
                );

                e.timestamp(Timestamp::now());

                e
            })
        })
        .await
}

async fn update_embed_page(
    ctx: &Context,
    msg: &mut Message,
    data: &[LuckymonHistory],
    items_per_page: usize,
    current_page: usize,
    original_owner: &Message
) -> serenity::Result<()> {
    let start_index = current_page * items_per_page;
    let end_index = usize::min(start_index + items_per_page, data.len());

    let current_data = &data[start_index..end_index];

    let mut pokedex_numbers = Vec::new(); 
    let mut pokemon_names = Vec::new(); 
    let mut dates = Vec::new(); 

    for hist in current_data {
        pokedex_numbers.push(hist.pokemon_id);
        dates.push(hist.date_obtained);

        let mut name = hist.pokemon_name.clone();
        if hist.shiny {
            name = format!("✨{}✨", hist.pokemon_name.clone())
        }
        pokemon_names.push(name);
    }
    let mut total_pages = (data.len() as f64 / items_per_page as f64).ceil() as usize;
    if total_pages == 0 {
        total_pages = 1;
    }

    let author_name = &original_owner.author.name.clone();
    let avatar_url = &original_owner.author.avatar_url().unwrap().clone();

    msg.edit(&ctx.http, |m| {
        m.embed(|e| {
            e.title("Luckymon History")
                .color(Colour::from_rgb(0, 255, 255))
                .footer(|f| {
                    f.text(
                        format!(
                            "{}: Page {} of {}",
                            author_name,
                            current_page + 1,
                            total_pages
                        )
                    );
                    f.icon_url(avatar_url)
                });

            e.field(
                "Pokédex #",
                pokedex_numbers.iter().map(|s| s.to_string()).collect::<Vec<String>>().join("\n"),
                true,
            );

            e.field(
                "Pokémon's Name",
                pokemon_names.iter().map(|s| s.to_string()).collect::<Vec<String>>().join("\n"),
                true,
            );

            e.field(
                "Obtained (YYYY-MM-DD)",
                dates.iter().map(|s| s.to_string()).collect::<Vec<String>>().join("\n"),
                true,
            );

            e.timestamp(Timestamp::now());

            e
        })
    })
    .await
}

