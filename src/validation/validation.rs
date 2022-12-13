use serenity::prelude::*;
use serenity::model::channel::Message;
use serenity::model::guild::Role;
use serenity::http::CacheHttp;

pub async fn has_corkboard_role(ctx: &Context, msg: &Message) -> bool {
    let guild_id = msg.guild_id.unwrap();
    let roles : Vec<Role> = ctx.http().get_guild_roles(guild_id.into()).await.unwrap();
    for role in roles {
        if role.name == "corkboard" {
            if msg.author.has_role(&ctx.http, guild_id, role.id).await.unwrap() {
                return true;
            } else {
                let _msg = msg
                    .channel_id.say(
                        &ctx.http,
                        ":bangbang: Error :bangbang: - Only users with the `corkboard` role can execute this command."
                    )
                    .await;
                return false;
            }
        }
    }

    false
}

pub async fn has_correct_arg_count(
    ctx: &Context,
    msg: &Message,
    expected: usize,
    actual: usize,
    args: Vec<&str>,
    command_name: &str
) -> bool {
    if expected != actual {
        let _msg = msg
            .channel_id.say(
                &ctx.http,
                format!(
                    ":bangbang: Error :bangbang: - the `{}` command requires {} arguments:\n\n\t\t{:?}
                    \nSee `.help {}`  for more usage details.",
                    command_name,
                    expected,
                    args,
                    command_name
                )
            )
            .await;

        return false;
    }

    true
}
