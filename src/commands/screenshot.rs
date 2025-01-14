use std::io::Write;

use async_trait::async_trait;
use reqwest::Url;
use tdlib::enums::{InputFile, InputMessageContent};
use tdlib::types::{FormattedText, InputFileLocal, InputMessagePhoto};
use tempfile::NamedTempFile;
use url::ParseError;

use super::CommandError::CustomFormattedText;
use super::{CommandResult, CommandTrait};
use crate::apis::microlink;
use crate::utilities::command_context::CommandContext;
use crate::utilities::convert_argument::{ConvertArgument, StringGreedyOrReply};
use crate::utilities::message_entities::{self, ToEntity};
use crate::utilities::rate_limit::RateLimiter;

pub struct Screenshot;

#[async_trait]
impl CommandTrait for Screenshot {
    fn command_names(&self) -> &[&str] {
        &["screenshot", "ss", "webimg", "webimage", "webscreenshot"]
    }

    fn description(&self) -> Option<&'static str> {
        Some("screenshot a webpage")
    }

    fn rate_limit(&self) -> RateLimiter<i64> {
        RateLimiter::new(3, 60)
    }

    async fn execute(&self, ctx: &CommandContext, arguments: String) -> CommandResult {
        let StringGreedyOrReply(url) = ConvertArgument::convert(ctx, &arguments).await?.0;

        let url = match Url::parse(&url) {
            Err(ParseError::RelativeUrlWithoutBase) => Url::parse(&format!("http://{url}")),
            url => url,
        };

        ctx.send_typing().await?;

        let data = microlink::screenshot(
            ctx.bot_state.http_client.clone(),
            url.map_err(|err| err.to_string())?,
        )
        .await?
        .map_err(|err| {
            CustomFormattedText(message_entities::formatted_text(vec![
                err.code.text_url(&err.more),
                ": ".text(),
                err.message.text(),
            ]))
        })?;

        let screenshot = ctx
            .bot_state
            .http_client
            .get(data.screenshot.url)
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(&screenshot).unwrap();

        let message = ctx
            .reply_custom(
                InputMessageContent::InputMessagePhoto(InputMessagePhoto {
                    photo: InputFile::Local(InputFileLocal {
                        path: temp_file.path().to_str().unwrap().into(),
                    }),
                    thumbnail: None,
                    added_sticker_file_ids: Vec::new(),
                    width: 0,
                    height: 0,
                    caption: data.title.map(|t| FormattedText { text: t, ..Default::default() }),
                    self_destruct_time: 0,
                    has_spoiler: false,
                }),
                None,
            )
            .await?;

        ctx.bot_state.message_queue.wait_for_message(message.id).await?;
        temp_file.close().unwrap();

        Ok(())
    }
}
