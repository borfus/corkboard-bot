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
                return false;
            }
        }
    }

    false
}

