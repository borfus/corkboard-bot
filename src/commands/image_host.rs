//! Parks a generated image in a staging channel and hands back its CDN URL.
//!
//! Interaction responses cannot be edited to carry a new attachment -- serenity
//! 0.11's `EditInteractionResponse` has no file support -- so any command that
//! wants a picture *and* a deferred or updatable reply has to reference the
//! image by URL instead of by `attachment://`.
//!
//! This also removes a whole class of fragility: an embed pointing at a stable
//! URL survives every edit, where an embed pointing at its own attachment can
//! come unstuck and leave the upload showing twice.
//!
//! Lifted out of `luckydex`, which has worked this way for a while, so every
//! image-bearing command can share it.

use std::io::Cursor;

use image::codecs::png::PngEncoder;
use image::{ImageBuffer, ImageEncoder, Rgba};
use serenity::model::id::{ChannelId, GuildId};
use serenity::model::prelude::AttachmentType;
use serenity::prelude::*;

// If you host this bot yourself, this needs to be a server and channel the bot
// can post in -- it is only ever used to turn bytes into a URL.
const STAGING_CHANNEL: ChannelId = ChannelId(1155366534617763931);
const STAGING_GUILD: GuildId = GuildId(423944755118866444);

pub fn encode_png(img: &ImageBuffer<Rgba<u8>, Vec<u8>>) -> Vec<u8> {
    let mut buffer: Vec<u8> = Vec::new();
    {
        let mut writer = Cursor::new(&mut buffer);
        let encoder = PngEncoder::new(&mut writer);
        let _ = encoder.write_image(img, img.width(), img.height(), image::ColorType::Rgba8);
    }
    buffer
}

/// Uploads raw PNG bytes and returns the attachment URL.
///
/// Returns an empty string when the staging channel is unreachable; callers
/// treat that as "no image" rather than failing the whole command.
pub async fn upload(ctx: &Context, buffer: &Vec<u8>) -> String {
    let mut image_url = String::new();

    if let Ok(channel) = STAGING_CHANNEL.to_channel(&ctx).await {
        if let Some(guild_channel) = channel.guild() {
            if guild_channel.guild_id == STAGING_GUILD {
                let files = vec![AttachmentType::Bytes {
                    data: buffer.into(),
                    filename: "image.png".to_string(),
                }];
                if let Ok(sent_message) = STAGING_CHANNEL
                    .send_files(&ctx.http, files, |m| m.content(""))
                    .await
                {
                    if let Some(attachment) = sent_message.attachments.first() {
                        image_url = attachment.url.clone();
                    }
                }
            }
        }
    }

    image_url
}

/// Encode-and-upload in one step, which is what every caller actually wants.
pub async fn host(ctx: &Context, img: &ImageBuffer<Rgba<u8>, Vec<u8>>) -> String {
    let bytes = encode_png(img);
    upload(ctx, &bytes).await
}
