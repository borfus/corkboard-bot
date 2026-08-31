//! Slash command plumbing.
//!
//! Everything the bot exposes is an application command. The prefix framework
//! is gone: a prefix command's reply is an ordinary channel message, which can
//! never be ephemeral, and its arguments are posted publicly for everyone to
//! read -- which is exactly what leaked a player's team before a battle.
//!
//! Command bodies live with their feature, in `commands::*`. This module owns
//! the boundary: declaring the commands to Discord, routing an incoming
//! interaction to the right handler, and the small helpers that read options
//! and send replies.

pub mod registry;

use serenity::builder::CreateEmbed;
use serenity::model::application::interaction::application_command::{
    ApplicationCommandInteraction, CommandDataOptionValue,
};
use serenity::model::application::interaction::InteractionResponseType;
use serenity::model::id::{GuildId, UserId};
use serenity::model::user::User;
use serenity::prelude::*;

/// One invocation of a slash command, with the option-reading and replying
/// helpers the command bodies need.
pub struct Invocation<'a> {
    pub ctx: &'a Context,
    pub command: &'a ApplicationCommandInteraction,
}

impl<'a> Invocation<'a> {
    pub fn new(ctx: &'a Context, command: &'a ApplicationCommandInteraction) -> Self {
        Invocation { ctx, command }
    }

    pub fn user(&self) -> &User {
        &self.command.user
    }

    pub fn user_id(&self) -> UserId {
        self.command.user.id
    }

    pub fn guild_id(&self) -> Option<GuildId> {
        self.command.guild_id
    }

    /// How the caller looks *in this server*: their nickname and their
    /// per-guild avatar, falling back to the global profile only where the
    /// server-specific one is unset.
    ///
    /// Both halves matter. The interaction payload carries `member.nick`, but
    /// not the guild avatar, so this reads the member record -- served from
    /// cache when the guild is cached -- rather than reaching for
    /// `user.avatar_url()`, which is always the global picture.
    pub async fn author_identity(&self) -> (String, Option<String>) {
        if let Some(guild_id) = self.guild_id() {
            if let Ok(member) = guild_id.member(&self.ctx.http, self.user_id()).await {
                let name = member.display_name().to_string();
                // Guild avatar first; a member without one shows their global.
                let avatar = member.avatar_url().or_else(|| member.user.avatar_url());
                return (name, avatar);
            }
        }

        (
            self.command.user.name.clone(),
            self.command.user.avatar_url(),
        )
    }

    fn option(&self, name: &str) -> Option<&CommandDataOptionValue> {
        self.command
            .data
            .options
            .iter()
            .find(|o| o.name == name)
            .and_then(|o| o.resolved.as_ref())
    }

    pub fn string(&self, name: &str) -> Option<String> {
        match self.option(name) {
            Some(CommandDataOptionValue::String(s)) => Some(s.clone()),
            _ => None,
        }
    }

    pub fn integer(&self, name: &str) -> Option<i64> {
        match self.option(name) {
            Some(CommandDataOptionValue::Integer(i)) => Some(*i),
            _ => None,
        }
    }

    pub fn user_arg(&self, name: &str) -> Option<UserId> {
        match self.option(name) {
            Some(CommandDataOptionValue::User(u, _)) => Some(u.id),
            _ => None,
        }
    }

    /// Acknowledges the interaction so the three second window stops running.
    ///
    /// Every command here talks to the API server before it can say anything
    /// useful, so this is the first thing each one does.
    pub async fn defer(&self, ephemeral: bool) -> serenity::Result<()> {
        self.command
            .create_interaction_response(&self.ctx.http, |r| {
                r.kind(InteractionResponseType::DeferredChannelMessageWithSource)
                    .interaction_response_data(|d| d.ephemeral(ephemeral))
            })
            .await
    }

    /// Fills in the deferred response with an embed.
    pub async fn embed(&self, embed: CreateEmbed) -> serenity::Result<()> {
        self.command
            .edit_original_interaction_response(&self.ctx.http, |r| {
                r.set_embed(embed);
                r
            })
            .await
            .map(|_| ())
    }

    /// Fills in the deferred response with plain text. Used for errors, which
    /// read better without embed chrome around them.
    pub async fn text(&self, content: impl Into<String>) -> serenity::Result<()> {
        let content: String = content.into();
        self.command
            .edit_original_interaction_response(&self.ctx.http, |r| r.content(&content))
            .await
            .map(|_| ())
    }

    /// Reports a failure. Deliberately the same shape everywhere so a user can
    /// tell "the bot said no" from "the bot fell over".
    pub async fn fail(&self, message: impl std::fmt::Display) -> serenity::Result<()> {
        self.text(format!(":bangbang: {}", message)).await
    }

    /// Replies immediately with an embed and a local file behind it.
    ///
    /// Not deferred, because `EditInteractionResponse` in serenity 0.11 cannot
    /// carry attachments -- the file has to ride along on the first response.
    /// That means the work behind these commands has to finish inside Discord's
    /// three second window, which is fine against a local API server.
    pub async fn reply_with_file(
        &self,
        embed: CreateEmbed,
        file_path: &str,
        ephemeral: bool,
    ) -> serenity::Result<()> {
        self.command
            .create_interaction_response(&self.ctx.http, |r| {
                r.kind(InteractionResponseType::ChannelMessageWithSource)
                    .interaction_response_data(|d| {
                        d.add_file(std::path::Path::new(file_path))
                            .ephemeral(ephemeral)
                            .set_embed(embed)
                    })
            })
            .await
    }

    /// Replies immediately with an embed, no deferral.
    ///
    /// For commands with nothing to fetch first -- answering straight away
    /// skips the "Bot is thinking..." flash entirely.
    pub async fn reply_embed(&self, embed: CreateEmbed, ephemeral: bool) -> serenity::Result<()> {
        self.command
            .create_interaction_response(&self.ctx.http, |r| {
                r.kind(InteractionResponseType::ChannelMessageWithSource)
                    .interaction_response_data(|d| d.ephemeral(ephemeral).set_embed(embed))
            })
            .await
    }

    /// Immediate plain-text failure, for commands that reply without deferring.
    pub async fn fail_now(
        &self,
        message: impl std::fmt::Display,
        ephemeral: bool,
    ) -> serenity::Result<()> {
        self.command
            .create_interaction_response(&self.ctx.http, |r| {
                r.kind(InteractionResponseType::ChannelMessageWithSource)
                    .interaction_response_data(|d| {
                        d.ephemeral(ephemeral)
                            .content(format!(":bangbang: {}", message))
                    })
            })
            .await
    }
}

/// A user's name in this server, or `None` if they are not in it any more.
///
/// Membership check and name lookup in one call: the leaderboard needs both,
/// and asking twice would double the round trips for no reason. Served from
/// cache for anyone the bot has already seen here.
pub async fn present_member_name(
    ctx: &Context,
    guild_id: GuildId,
    user_id: UserId,
) -> Option<String> {
    guild_id
        .member(&ctx.http, user_id)
        .await
        .ok()
        .map(|member| member.display_name().to_string())
}

/// How some *other* user looks in this server: their nickname if they have one,
/// otherwise their account name.
///
/// The caller's own identity comes from `Invocation::author_identity`, which
/// also carries the avatar. This is for everyone else -- opponents, battle
/// participants -- where only a name is rendered and they are known to be here.
pub async fn member_display_name(ctx: &Context, guild_id: GuildId, user_id: UserId) -> String {
    if let Some(name) = present_member_name(ctx, guild_id, user_id).await {
        return name;
    }
    user_id
        .to_user(&ctx.http)
        .await
        .map(|u| u.name)
        .unwrap_or_else(|_| "Trainer".to_string())
}

/// Commands only make sense inside a guild -- everything is scoped by guild_id,
/// including the leaderboards.
pub async fn require_guild(inv: &Invocation<'_>) -> Option<GuildId> {
    match inv.guild_id() {
        Some(g) => Some(g),
        None => {
            let _ = inv.fail("This command only works inside a server.").await;
            None
        }
    }
}
