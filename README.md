# Summary

Corkboard-bot is the front-end client for the Corkboard project, a Discord bot that interacts with the [Corkboard Server](https://github.com/borfus/corkboard-server) to create, read, update, and delete "Events", "Pins", and "FAQs".

Run various commands using the `.` prefix. For a list of commands, run `.help`.

Corkboard bot uses the [Serenity](https://github.com/serenity-rs/serenity) Rust Discord bot framework.

# Requirements

Corkboard-bot interacts with the [Corkboard Server](https://github.com/borfus/corkboard-server) project and must be installed and running on the same machine as the bot.

You can build and run the project using Cargo, Rust's official dependency management and build tool. 

# Usage

## Available Commands

General:

- `list`
- `pins`
- `events` 
- `faqs` 
- `luckymon` 

Admin (Requires the `corkboard` role to run):

- `add_faq` 
- `edit_faq` 
- `delete_faq` 
- `add_event` 
- `edit_event` 
- `delete_event` 
- `add_pin` 
- `edit_pin` 
- `delete_pin` 
 
## Example Usage

Corkboard bot uses a simple `command args` format. For example, if I want to add a FAQ, I would run the `add_faq` command followed by the required number of arguments in quotes:

`.add_faq "This is a question?" "This is the answer!"`

If successful, Corkboard bot will respond with details on the newly created FAQ.

The format is the same for all create and edit commands. One additional point of interest is dates.

At the moment, all date data is based off of the `America/Los_Angeles` time zone. The working format for dates is as follows: `MM/DD/YYYY HH:mm[AM/PM]`

An example of this would be when editing an event:

`.edit_event 1 "Title of event" "https://www.event-url.com/" "Description of event" "12/1/2022 9:00AM" "12/5/2022 11:30PM"`

## Help

If you require help for a specific command and a list of its arguments, type `.help [command_name]`.

