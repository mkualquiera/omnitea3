#![deny(clippy::pedantic)]

use std::env;
use std::fs::File;
use std::path::Path;

mod openai;

use openai::{ChatLog, OpenAI};

use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::model::prelude::{AttachmentType, Channel};
use serenity::prelude::*;

use log::{debug, error, info};

fn setup_logger() -> Result<(), fern::InitError> {
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "{}[{}][{}] {}",
                chrono::Local::now().format("[%Y-%m-%d][%H:%M:%S]"),
                record.target(),
                record.level(),
                message
            ));
        })
        .level(log::LevelFilter::Debug)
        .chain(std::io::stdout())
        //.chain(fern::log_file("output.log")?)
        // remove targets different than the current one
        .filter(|metadata| metadata.target().starts_with(env!("CARGO_PKG_NAME")))
        .apply()?;
    Ok(())
}

// Constant for the maximum number of tokens in a chat log
const MAX_TOKENS: usize = 4096 - 500;

/// A part of the bot response, which can be text or an image
enum BotResponse {
    /// The text of the chunk
    Text(String),
    /// The paths of the images
    Image(Vec<String>, String),
}

use regex::Regex;

/// Take a response message and turn it into a parsed response
fn parse_response(response: String) -> BotResponse {
    // Check which lines contain \$([^$]+)\$
    let re = Regex::new(r"\$([^$]+)\$").unwrap();

    // See if there is at least one match
    if re.is_match(&response) {
        // Return the images
        render_md(&response)
    } else {
        // Return the text
        BotResponse::Text(response)
    }
}

use std::io::Write;
use std::process::Command;

/// Takes a string, and renders it as markdown to a temporary file and returns the path
/// to the file. It uses pandoc to render the markdown, and then imagemagick to convert
/// the pdf to a png. There may be many files as output, so it returns a vector of paths.
fn render_md(markdown: &str) -> BotResponse {
    let fixed_markdown = markdown.to_string();

    // Create a file with a random name
    let filenum = rand::random::<u64>().to_string();
    let name = format!("{filenum}.md");
    // Open the file in the current directory
    let mut file = File::create(&name).unwrap();

    // Write \pagenumbering{gobble}\n to the file
    file.write_all(b"\\pagenumbering{gobble}\n").unwrap();

    // Write the markdown to the file
    file.write_all(fixed_markdown.as_bytes()).unwrap();

    // Flush the file
    file.flush().unwrap();

    // Run pandoc to convert the markdown to a pdf
    let output = Command::new("pandoc")
        .arg("-V")
        .arg("geometry:margin=0.2in")
        .arg("-V")
        .arg("geometry:paperwidth=4.25in")
        .arg("-V")
        .arg("geometry:paperheight=3.25in")
        .arg("--pdf-engine=xelatex")
        .arg("-o")
        .arg(&format!("{filenum}.pdf"))
        .arg(&name)
        .output()
        .expect("failed to execute pandoc");

    // Check if the command failed
    if !output.status.success() {
        // Print the error
        println!("pandoc failed: {}", String::from_utf8_lossy(&output.stderr));
    }

    // Run imagemagick to convert the pdf to a png
    Command::new("convert")
        .arg("-trim")
        .arg("-density")
        .arg("300")
        .arg("-channel")
        .arg("RGB")
        .arg("-negate")
        .arg("+channel")
        .arg("RGB")
        .arg(&format!("{filenum}.pdf"))
        .arg(&format!("{filenum}.png"))
        .output()
        .expect("failed to execute convert");

    // Get all the png files that were created. They are named {filenum}-{number}.png
    let mut paths = Vec::new();

    // Get the current directory
    let path = Path::new(".");

    let entries = path.read_dir().unwrap();

    // Sort the entries by name
    let mut entries: Vec<_> = entries.collect();
    entries.sort_by_key(|a| a.as_ref().unwrap().path());

    // Iterate over all the files in the directory
    for entry in &entries {
        // Get the path of the file
        let path = entry.as_ref().unwrap().path();

        let extension = path.extension();

        // Check if the file is a png file
        if extension.is_some() && path.extension().unwrap() == "png" {
            // Check if the file starts with the filenum
            if path
                .file_stem()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with(&filenum)
            {
                // Add the path to the vector
                paths.push(path.to_str().unwrap().to_string());
            }
        }
    }

    BotResponse::Image(paths, markdown.to_string())
}

struct Handler {
    openai: OpenAI,
}

async fn add_user_message(
    ctx: Context,
    chat_log: ChatLog,
    message: &Message,
) -> ChatLog {
    // We may or may not be in a guild, so we need to handle that
    let user_nickname = match message.guild_id {
        Some(guild_id) => message
            .author
            .nick_in(&ctx.http, guild_id)
            .await
            .unwrap_or_else(|| message.author.name.clone()),
        None => message.author.name.clone(),
    };

    let mut content = message.content.clone();

    // Check if the message has a file attached, and add them to the content
    if !message.attachments.is_empty() {
        let attachments = message
            .attachments
            .iter()
            .map(|a| a.url.clone())
            .collect::<Vec<String>>();

        for attachment in attachments {
            let attachment_string = reqwest::get(&attachment)
                .await
                .unwrap()
                .text()
                .await
                .unwrap();

            let filename = attachment.split('/').last().unwrap();

            content.push_str(&format!("File {filename}: \n{attachment_string}"));
        }
    }

    chat_log.user(&format!("{user_nickname} says: {content}"))
}

async fn add_message(ctx: Context, chat_log: ChatLog, message: &Message) -> ChatLog {
    // we need to check if the id of the author is the same as the id of the bot
    if message.is_own(&ctx.cache) {
        chat_log.assistant(&message.content)
    } else {
        add_user_message(ctx, chat_log, message).await
    }
}

async fn build_chat_log(
    ctx: Context,
    messages: Vec<Message>,
    prompt: Option<String>,
) -> ChatLog {
    let mut chat_log = ChatLog::new();

    let prompt = if let Some(user_prompt) = prompt {
        user_prompt
    } else {
        include_str!(env!("PROMPT_FILE")).to_owned()
    };

    for (i, message) in messages.clone().into_iter().enumerate() {
        // See if this is the fourth to last message, or if there are less than 4 messages
        if i == messages.len() - 4 || messages.len() < 4 {
            // If it is, we need to add the user message
            chat_log = chat_log.system(&prompt);
        }
        chat_log = add_message(ctx.clone(), chat_log, &message).await;
    }

    chat_log
}

/// Function that sends a message and splits it into multiple messages if it is too long
async fn send_message(
    ctx: Context,
    original_message: Message,
    message: String,
    escape: bool,
) {
    // Split the message into multiple messages if it is too long
    let chars = message.as_str().chars().collect::<Vec<char>>();
    // Do chunks of 2000 - 6 to account for the code block
    let chunks = chars.chunks(2000 - 6);

    // Iterate over the chunks
    for chunk in chunks {
        // Convert the chunk to a string
        let chunk = chunk.iter().collect::<String>();

        // If we need to escape the message
        let chunk = if escape {
            // Escape the message
            format!("```{chunk}```")
        } else {
            chunk
        };

        // Send the message
        if let Err(why) = original_message.channel_id.say(&ctx.http, chunk).await {
            error!("Error sending message: {:?}", why);
        }
    }
}

async fn fetch_included_messages(ctx: Context, msg: Message) -> ChatLog {
    let mut messages_to_include = Vec::new();
    messages_to_include.push(msg.clone());

    let mut user_prompt = None;

    // Add past messages until we go over the limit
    loop {
        let past_messages = msg
            .channel_id
            .messages(&ctx.http, |retriever| {
                retriever
                    .before(messages_to_include.first().unwrap().id)
                    .limit(10)
            })
            .await
            .unwrap();

        if past_messages.is_empty() {
            break;
        }

        let mut found_barrier = false;

        // Add them at the start of the vector
        for message in past_messages {
            // See if the message is a barrier
            if message.content.starts_with("|b|") {
                debug!("Barrier found, stopping");
                found_barrier = true;

                // Get the rest of the text for the user prompt
                let remainder = message.content[3..].trim();

                if !remainder.is_empty() {
                    user_prompt = Some(remainder.to_string());
                }

                break;
            }
            // See if the message is an aside
            if message.content.starts_with("|a|") {
                debug!("Aside found, skipping");
                continue;
            }
            messages_to_include.insert(0, message.clone());
        }

        // Count the number of tokens in the chat log
        let chat_log = build_chat_log(
            ctx.clone(),
            messages_to_include.clone(),
            user_prompt.clone(),
        )
        .await;

        let tokens = chat_log.count_tokens();
        if tokens > MAX_TOKENS || found_barrier {
            break;
        }
    }

    // Remove messages until we are under the limit
    while messages_to_include.len() > 1 {
        let chat_log = build_chat_log(
            ctx.clone(),
            messages_to_include.clone(),
            user_prompt.clone(),
        )
        .await;

        let tokens = chat_log.count_tokens();
        if tokens <= MAX_TOKENS {
            break;
        }

        messages_to_include.remove(0);
    }

    build_chat_log(ctx, messages_to_include, user_prompt).await
}

#[async_trait]
impl EventHandler for Handler {
    // Set a handler for the `message` event - so that whenever a new message
    // is received - the closure (or function) passed will be called.
    //
    // Event handlers are dispatched through a threadpool, and so multiple
    // events can be dispatched simultaneously.
    async fn message(&self, ctx: Context, msg: Message) {
        // Ignore messages from self
        if msg.is_own(&ctx.cache) {
            return;
        }

        // The message has to either be in a channel called "omnitea" or in a DM
        let channel = msg.channel_id.to_channel(&ctx).await.unwrap();

        // Get channel name from environment variable
        let target_channel =
            env::var("CHANNEL_NAME").unwrap_or_else(|_| "omnitea".to_string());

        match channel {
            Channel::Guild(channel) => {
                if channel.name != target_channel {
                    return;
                }
            }
            Channel::Private(_) => {}
            _ => return,
        }

        info!("Received message: {}", msg.content);
        // See if the message is a barrier
        if msg.content.starts_with("|b|") {
            info!("Barrier received");

            // React with a checkmark
            if let Err(why) = msg.react(&ctx.http, '✅').await {
                error!("Error reacting: {:?}", why);
            }
            return;
        }
        // See if the message received is an aside, and ignore it if so
        if msg.content.starts_with("|a|") {
            info!("Aside received");

            // React with a silent checkmark
            if let Err(why) = msg.react(&ctx.http, '🔇').await {
                error!("Error reacting: {:?}", why);
            }
            return;
        }

        // Get the messages to include
        let chat_log = fetch_included_messages(ctx.clone(), msg.clone()).await;

        debug!("Chat log: {:?}", chat_log);
        info!("Context length: {}", chat_log.count_tokens());

        // Start the "typing" indicator
        let typing = msg.channel_id.start_typing(&ctx.http);

        let completion = chat_log.complete(&self.openai).await;
        debug!("Completion: {:?}", completion);

        match completion {
            Ok(completion) => {
                // Parse the completion
                let response = parse_response(completion.content);

                match response {
                    BotResponse::Text(text) => {
                        // Send the response
                        send_message(ctx, msg, text, false).await;
                    }
                    BotResponse::Image(path_strs, original_text) => {
                        for path_str in path_strs {
                            let path = Path::new(&path_str);
                            // Send as an attachment
                            if let Err(why) = msg
                                .channel_id
                                .send_message(&ctx.http, |m| {
                                    m.add_file(AttachmentType::Path(path));
                                    m
                                })
                                .await
                            {
                                error!("Error sending message: {:?}", why);
                            }
                        }

                        send_message(ctx, msg, original_text, true).await;
                    }
                }
            }
            Err(why) => {
                error!("Error completing chat: {:?}", why);
            }
        }

        // Stop the "typing" indicator
        match typing {
            Ok(typing) => {
                let _ = typing.stop();
            }
            Err(why) => {
                error!("Error stopping typing: {:?}", why);
            }
        }
    }

    // Set a handler to be called on the `ready` event. This is called when a
    // shard is booted, and a READY payload is sent by Discord. This payload
    // contains data like the current user's guild Ids, current user data,
    // private channels, and more.
    //
    // In this case, just print what the current user's username is.
    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}

#[tokio::main]
async fn main() {
    // Configure logging
    setup_logger().expect("Failed to setup logging");
    // Configure the client with your Discord bot token in the environment.
    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");
    let openai_key = env::var("OPENAI_KEY").expect("Expected a key in the environment");
    // Set gateway intents, which decides what events the bot will be notified about
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;

    // Create a new instance of the Client, logging in as a bot. This will
    // automatically prepend your bot token with "Bot ", which is a requirement
    // by Discord for bot users.
    let mut client = Client::builder(&token, intents)
        .event_handler(Handler {
            openai: OpenAI::new(openai_key),
        })
        .await
        .expect("Err creating client");

    // Finally, start a single shard, and start listening to events.
    //
    // Shards will automatically attempt to reconnect, and will perform
    // exponential backoff until it reconnects.
    if let Err(why) = client.start().await {
        println!("Client error: {why:?}");
    }
}
