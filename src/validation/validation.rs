use crate::slash::Invocation;

/// Gate for the admin commands.
///
/// The interaction already carries the caller's role ids, so this only needs to
/// look up which role is called `corkboard` -- no second round trip to ask
/// whether the user has it.
pub async fn has_corkboard_role(inv: &Invocation<'_>) -> bool {
    let guild_id = match inv.guild_id() {
        Some(g) => g,
        None => {
            let _ = inv.fail("This command only works inside a server.").await;
            return false;
        }
    };

    let member = match inv.command.member.as_ref() {
        Some(m) => m,
        None => {
            let _ = inv.fail("Could not read your roles.").await;
            return false;
        }
    };

    let roles = match inv.ctx.http.get_guild_roles(guild_id.into()).await {
        Ok(r) => r,
        Err(_) => {
            let _ = inv.fail("Could not read this server's roles.").await;
            return false;
        }
    };

    let corkboard = roles.iter().find(|r| r.name == "corkboard");

    match corkboard {
        Some(role) if member.roles.contains(&role.id) => true,
        _ => {
            let _ = inv
                .fail("Only users with the `corkboard` role can run this command.")
                .await;
            false
        }
    }
}

// `has_correct_arg_count` used to live here. Discord validates required options
// before the interaction ever reaches the bot, so a command body can no longer
// be called with the wrong number of arguments and the check has nothing left
// to do.
