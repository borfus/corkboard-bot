//! Declaring the commands to Discord, and routing invocations back.
//!
//! Registration is per guild rather than global: guild commands appear the
//! instant the bot starts, where global ones can take up to an hour to
//! propagate. The bot re-declares its full set on every ready, so the list is
//! whatever this file says it is.

use serenity::model::application::command::CommandOptionType;
use serenity::model::application::interaction::application_command::ApplicationCommandInteraction;
use serenity::model::id::GuildId;
use serenity::prelude::*;

use crate::commands;
use crate::slash::Invocation;

/// Every command this bot answers to.
///
/// The canonical list, kept next to the definitions below so the two are read
/// together. Nothing at runtime reads it -- it exists so `/help` can be checked
/// against it, which is what stops a new command from quietly going
/// undocumented.
#[allow(dead_code)]
pub const COMMANDS: [&str; 19] = [
    "help",
    "pins",
    "events",
    "faqs",
    "luckymon",
    "luckydex",
    "luckyteam",
    "luckyboard",
    "luckybattle",
    "luckytrade",
    "add-pin",
    "edit-pin",
    "delete-pin",
    "add-faq",
    "edit-faq",
    "delete-faq",
    "add-event",
    "edit-event",
    "delete-event",
];

/// Admin commands still gate on the `corkboard` role at runtime. Discord's own
/// `default_member_permissions` is a blunter tool -- it keys off permissions,
/// not roles -- so the existing role check stays the source of truth.
pub async fn register(ctx: &Context, guild_id: GuildId) -> serenity::Result<()> {
    guild_id
        .set_application_commands(&ctx.http, |commands| {
            commands
                .create_application_command(|c| {
                    c.name("help")
                        .description("What the bot can do, and how to ask for it.")
                })
                .create_application_command(|c| {
                    c.name("pins").description("Retrieves all pins.")
                })
                .create_application_command(|c| {
                    c.name("events")
                        .description("Retrieves all events. All times in Pacific.")
                })
                .create_application_command(|c| {
                    c.name("faqs").description("Retrieves all FAQs.")
                })
                .create_application_command(|c| {
                    c.name("luckymon").description("Lucky pokemon of the day!")
                })
                .create_application_command(|c| {
                    c.name("luckydex")
                        .description("Your luckymon collection. Only you can see it.")
                })
                .create_application_command(|c| {
                    c.name("luckyteam")
                        .description("View or set your luckybattle team. Only you can see it.")
                        .create_option(|o| {
                            o.name("team")
                                .description("Up to 3 pokedex numbers, e.g. \"6 9 143\" (add s for shiny: 143s)")
                                .kind(CommandOptionType::String)
                                .required(false)
                        })
                })
                .create_application_command(|c| {
                    c.name("luckyboard")
                        .description("Battle standings for this server.")
                })
                .create_application_command(|c| {
                    c.name("luckybattle")
                        .description("Battle your luckymon against a trainer or another player.")
                        .create_option(|o| {
                            o.name("opponent")
                                .description("Who to challenge. Leave empty to fight an NPC trainer.")
                                .kind(CommandOptionType::User)
                                .required(false)
                        })
                        // A word rather than a True/False picker: "practice"
                        // says what it does, where "practice: True" makes you
                        // answer a question you did not ask. Discord names every
                        // option, so this reads `/luckybattle mode:Practice`.
                        .create_option(|o| {
                            o.name("mode")
                                .description("Ranked counts toward the leaderboard. Practice doesn't.")
                                .kind(CommandOptionType::String)
                                .required(false)
                                .add_string_choice("Ranked \u{2014} counts", "ranked")
                                .add_string_choice("Practice \u{2014} doesn't count", "practice")
                        })
                })
                .create_application_command(|c| {
                    c.name("luckytrade")
                        .description("Trade your luckymon with another user.")
                        .create_option(|o| {
                            o.name("user")
                                .description("Who you want to trade with")
                                .kind(CommandOptionType::User)
                                .required(true)
                        })
                        // Both optional: leaving one out is how you gift. The
                        // prefix command needed a literal "n/a" for that, which
                        // nothing on a slash command form would ever tell you.
                        .create_option(|o| {
                            o.name("offer")
                                .description("Pokemon you're giving: 123, or 123s for shiny. Leave empty to ask for a gift.")
                                .kind(CommandOptionType::String)
                                .required(false)
                        })
                        .create_option(|o| {
                            o.name("request")
                                .description("Pokemon you want back: 123, or 123s for shiny. Leave empty to give a gift.")
                                .kind(CommandOptionType::String)
                                .required(false)
                        })
                })
                // ---- pins ----
                .create_application_command(|c| {
                    c.name("add-pin")
                        .description("Add a pin.")
                        .create_option(|o| {
                            o.name("title")
                                .description("Pin title")
                                .kind(CommandOptionType::String)
                                .required(true)
                        })
                        .create_option(|o| {
                            o.name("url")
                                .description("Pin URL")
                                .kind(CommandOptionType::String)
                                .required(true)
                        })
                        .create_option(|o| {
                            o.name("description")
                                .description("Pin description")
                                .kind(CommandOptionType::String)
                                .required(true)
                        })
                })
                .create_application_command(|c| {
                    c.name("edit-pin")
                        .description("Edit a pin.")
                        .create_option(|o| {
                            o.name("id")
                                .description("Pin number, as listed by /pins")
                                .kind(CommandOptionType::Integer)
                                .required(true)
                        })
                        .create_option(|o| {
                            o.name("title")
                                .description("Pin title")
                                .kind(CommandOptionType::String)
                                .required(true)
                        })
                        .create_option(|o| {
                            o.name("url")
                                .description("Pin URL")
                                .kind(CommandOptionType::String)
                                .required(true)
                        })
                        .create_option(|o| {
                            o.name("description")
                                .description("Pin description")
                                .kind(CommandOptionType::String)
                                .required(true)
                        })
                })
                .create_application_command(|c| {
                    c.name("delete-pin")
                        .description("Delete a pin.")
                        .create_option(|o| {
                            o.name("id")
                                .description("Pin number, as listed by /pins")
                                .kind(CommandOptionType::Integer)
                                .required(true)
                        })
                })
                // ---- faqs ----
                .create_application_command(|c| {
                    c.name("add-faq")
                        .description("Create a new FAQ.")
                        .create_option(|o| {
                            o.name("question")
                                .description("The question")
                                .kind(CommandOptionType::String)
                                .required(true)
                        })
                        .create_option(|o| {
                            o.name("answer")
                                .description("The answer")
                                .kind(CommandOptionType::String)
                                .required(true)
                        })
                })
                .create_application_command(|c| {
                    c.name("edit-faq")
                        .description("Edit an existing FAQ.")
                        .create_option(|o| {
                            o.name("id")
                                .description("FAQ number, as listed by /faqs")
                                .kind(CommandOptionType::Integer)
                                .required(true)
                        })
                        .create_option(|o| {
                            o.name("question")
                                .description("The question")
                                .kind(CommandOptionType::String)
                                .required(true)
                        })
                        .create_option(|o| {
                            o.name("answer")
                                .description("The answer")
                                .kind(CommandOptionType::String)
                                .required(true)
                        })
                })
                .create_application_command(|c| {
                    c.name("delete-faq")
                        .description("Delete a FAQ.")
                        .create_option(|o| {
                            o.name("id")
                                .description("FAQ number, as listed by /faqs")
                                .kind(CommandOptionType::Integer)
                                .required(true)
                        })
                })
                // ---- events ----
                .create_application_command(|c| {
                    c.name("add-event")
                        .description("Add an event. All times in Pacific.")
                        .create_option(|o| {
                            o.name("title")
                                .description("Event title")
                                .kind(CommandOptionType::String)
                                .required(true)
                        })
                        .create_option(|o| {
                            o.name("url")
                                .description("Event URL")
                                .kind(CommandOptionType::String)
                                .required(true)
                        })
                        .create_option(|o| {
                            o.name("description")
                                .description("Event description")
                                .kind(CommandOptionType::String)
                                .required(true)
                        })
                        .create_option(|o| {
                            o.name("start")
                                .description("Start date/time in Pacific, e.g. 08/31/2026 5:00PM")
                                .kind(CommandOptionType::String)
                                .required(true)
                        })
                        .create_option(|o| {
                            o.name("end")
                                .description("End date/time in Pacific, e.g. 08/31/2026 9:00PM")
                                .kind(CommandOptionType::String)
                                .required(true)
                        })
                })
                .create_application_command(|c| {
                    c.name("edit-event")
                        .description("Edit an event. All times in Pacific.")
                        .create_option(|o| {
                            o.name("id")
                                .description("Event number, as listed by /events")
                                .kind(CommandOptionType::Integer)
                                .required(true)
                        })
                        .create_option(|o| {
                            o.name("title")
                                .description("Event title")
                                .kind(CommandOptionType::String)
                                .required(true)
                        })
                        .create_option(|o| {
                            o.name("url")
                                .description("Event URL")
                                .kind(CommandOptionType::String)
                                .required(true)
                        })
                        .create_option(|o| {
                            o.name("description")
                                .description("Event description")
                                .kind(CommandOptionType::String)
                                .required(true)
                        })
                        .create_option(|o| {
                            o.name("start")
                                .description("Start date/time in Pacific, e.g. 08/31/2026 5:00PM")
                                .kind(CommandOptionType::String)
                                .required(true)
                        })
                        .create_option(|o| {
                            o.name("end")
                                .description("End date/time in Pacific, e.g. 08/31/2026 9:00PM")
                                .kind(CommandOptionType::String)
                                .required(true)
                        })
                })
                .create_application_command(|c| {
                    c.name("delete-event")
                        .description("Delete an event.")
                        .create_option(|o| {
                            o.name("id")
                                .description("Event number, as listed by /events")
                                .kind(CommandOptionType::Integer)
                                .required(true)
                        })
                })
        })
        .await
        .map(|_| ())
}

/// Routes an invocation to its handler.
///
/// Which replies are ephemeral is decided here, at the point of dispatch, so
/// the policy is readable in one place rather than scattered through the
/// command bodies.
pub async fn dispatch(ctx: &Context, command: &ApplicationCommandInteraction) {
    let inv = Invocation::new(ctx, command);
    let name = command.data.name.as_str();

    println!("Got /{} command..", name);

    let result = match name {
        "help" => commands::help::slash_help(&inv).await,
        "pins" => commands::pins::slash_pins(&inv).await,
        "events" => commands::events::slash_events(&inv).await,
        "faqs" => commands::faqs::slash_faqs(&inv).await,
        "luckymon" => commands::luckymon::slash_luckymon(&inv).await,

        // Private by design: a luckydex in chat is noise for everyone else, and
        // a visible team is a scouting report before a PvP battle.
        "luckydex" => commands::luckydex::slash_luckydex(&inv).await,
        "luckyteam" => commands::luckyteam::slash_luckyteam(&inv).await,

        "luckyboard" => commands::luckyboard::slash_luckyboard(&inv).await,
        "luckybattle" => commands::luckybattle::slash_luckybattle(&inv).await,
        "luckytrade" => commands::luckytrade::slash_luckytrade(&inv).await,

        "add-pin" => commands::pins::slash_add_pin(&inv).await,
        "edit-pin" => commands::pins::slash_edit_pin(&inv).await,
        "delete-pin" => commands::pins::slash_delete_pin(&inv).await,

        "add-faq" => commands::faqs::slash_add_faq(&inv).await,
        "edit-faq" => commands::faqs::slash_edit_faq(&inv).await,
        "delete-faq" => commands::faqs::slash_delete_faq(&inv).await,

        "add-event" => commands::events::slash_add_event(&inv).await,
        "edit-event" => commands::events::slash_edit_event(&inv).await,
        "delete-event" => commands::events::slash_delete_event(&inv).await,

        other => {
            println!("Unknown command: /{}", other);
            Ok(())
        }
    };

    if let Err(why) = result {
        println!("Error handling /{}: {:?}", name, why);
    }

    println!("Finished processing /{} command!", name);
}
