//! `/help` -- what the bot can do.
//!
//! Discord's own command picker already shows names and one-line descriptions,
//! so this exists for the things the picker cannot: grouping related commands,
//! spelling out argument shapes, and saying which replies are private and which
//! need a role.
//!
//! Always ephemeral. Nobody else needs a copy of the manual in the channel.

use serenity::builder::CreateEmbed;

use crate::slash::Invocation;

/// One line of the manual: command, its arguments, and what it does.
///
/// `<angled>` arguments are required, `[square]` optional -- the same shorthand
/// Discord uses in its own picker.
pub struct HelpEntry {
    pub name: &'static str,
    pub args: &'static str,
    pub blurb: &'static str,
}

const fn entry(name: &'static str, args: &'static str, blurb: &'static str) -> HelpEntry {
    HelpEntry { name, args, blurb }
}

pub const LUCKYMON: [HelpEntry; 6] = [
    entry("luckymon", "", "Your lucky pokemon of the day. Resets at midnight UTC."),
    entry("luckydex", "", "Your whole collection, page by page. Only you can see it."),
    entry(
        "luckyteam",
        "[team]",
        "Show the three you'd bring to a battle, or set them: `team: 6 9 143` (add `s` for a shiny, e.g. `143s`). Only you can see it.",
    ),
    entry(
        "luckybattle",
        "[opponent] [mode]",
        "Fight an NPC trainer, or name an `opponent` to challenge a player. `mode: Practice` fights for fun without touching the leaderboard.",
    ),
    entry(
        "luckyboard",
        "",
        "Battle standings. Wins count everywhere you play; the board shows this server's players.",
    ),
    entry(
        "luckytrade",
        "<user> [offer] [request]",
        "Swap pokemon with someone. Leave `offer` empty to ask for a gift, or `request` empty to give one.",
    ),
];

pub const BOARD: [HelpEntry; 3] = [
    entry("pins", "", "Everything pinned to the corkboard."),
    entry("events", "", "Upcoming events, in Pacific time."),
    entry("faqs", "", "Frequently asked questions."),
];

pub const ADMIN: [HelpEntry; 9] = [
    entry("add-pin", "<title> <url> <description>", "Pin something new."),
    entry("edit-pin", "<id> <title> <url> <description>", "Change a pin."),
    entry("delete-pin", "<id>", "Remove a pin."),
    entry("add-faq", "<question> <answer>", "Add a FAQ."),
    entry("edit-faq", "<id> <question> <answer>", "Change a FAQ."),
    entry("delete-faq", "<id>", "Remove a FAQ."),
    entry(
        "add-event",
        "<title> <url> <description> <start> <end>",
        "Add an event. Dates look like `08/31/2026 5:00PM`, Pacific.",
    ),
    entry(
        "edit-event",
        "<id> <title> <url> <description> <start> <end>",
        "Change an event.",
    ),
    entry("delete-event", "<id>", "Remove an event."),
];

fn render(entries: &[HelpEntry]) -> String {
    entries
        .iter()
        .map(|e| {
            let signature = if e.args.is_empty() {
                format!("`/{}`", e.name)
            } else {
                format!("`/{} {}`", e.name, e.args)
            };
            format!("{}\n{}", signature, e.blurb)
        })
        .collect::<Vec<String>>()
        .join("\n\n")
}

pub async fn slash_help(inv: &Invocation<'_>) -> serenity::Result<()> {
    let mut embed = CreateEmbed::default();
    embed
        .title("Corkboard Commands")
        .description(
            "Type `/` in any channel to browse these with Discord's own picker \u{2014} it fills in the arguments for you.",
        )
        .field("\u{1F3AE} Luckymon", render(&LUCKYMON), false)
        .field("\u{1F4CC} Corkboard", render(&BOARD), false)
        .field(
            "\u{1F6E0}\u{FE0F} Admin \u{2014} needs the `corkboard` role",
            render(&ADMIN),
            false,
        )
        .footer(|f| f.text("<required>  [optional]  \u{2014}  only you can see this message"));

    inv.reply_embed(embed, true).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::slash::registry::COMMANDS;

    fn documented() -> Vec<&'static str> {
        LUCKYMON
            .iter()
            .chain(BOARD.iter())
            .chain(ADMIN.iter())
            .map(|e| e.name)
            .collect()
    }

    /// The drift this guards against: adding a command and forgetting that help
    /// exists. `COMMANDS` is the canonical list, so anything registered has to
    /// turn up here too.
    #[test]
    fn every_command_is_documented() {
        let documented = documented();

        for name in COMMANDS.iter() {
            if *name == "help" {
                continue; // documented by being the thing you just ran
            }
            assert!(
                documented.contains(name),
                "/{} is registered but missing from /help",
                name
            );
        }
    }

    /// ...and the reverse, so help cannot advertise something that is gone.
    #[test]
    fn help_does_not_invent_commands() {
        for name in documented() {
            assert!(
                COMMANDS.contains(&name),
                "/help lists /{}, which is not a registered command",
                name
            );
        }
    }

    #[test]
    fn entries_are_unique_and_populated() {
        let mut names = documented();
        let before = names.len();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), before, "a command is documented twice");

        for e in LUCKYMON.iter().chain(BOARD.iter()).chain(ADMIN.iter()) {
            assert!(!e.name.is_empty());
            assert!(!e.blurb.is_empty(), "/{} has no description", e.name);
        }
    }

    /// Signatures render with a leading slash, not the old prefix.
    #[test]
    fn signatures_use_slash_commands() {
        let text = render(&LUCKYMON);
        assert!(text.contains("`/luckymon`"));
        assert!(text.contains("`/luckytrade <user> [offer] [request]`"));
        assert!(!text.contains("`."), "no prefix-style command should remain");
    }
}
