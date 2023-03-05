use std::env;
use std::fs::File;
use std::path::Path;

mod openai;

use openai::{ChatLog, OpenAI};

use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::model::prelude::AttachmentType;
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
            ))
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
enum BotResponseChunk {
    /// The text of the chunk
    Text(String),
    /// A path to the image
    Image(String),
}

use regex::Regex;

/// Take a response message and turn it into chunks
fn chunk_response(response: String) -> Vec<BotResponseChunk> {
    let mut chunks = Vec::new();

    // First, split the response into lines
    let lines = response.lines();

    // Check which lines contain \$([^$]+)\$
    let re = Regex::new(r"\$([^$]+)\$").unwrap();

    for line in lines {
        // Check if the line contains a math expression
        if re.is_match(line) {
            // Render it as latex
            let path = render_latex(line);
            // Add the image as a chunk
            chunks.push(BotResponseChunk::Image(path));
        } else {
            // If the line is empty, skip it
            if line.is_empty() {
                continue;
            }
            // Add the line as a text chunk
            chunks.push(BotResponseChunk::Text(line.to_string()));
        }
    }

    merge_chunks(chunks)
}

/// Efficiently merge neighboring text chunks into one
fn merge_chunks(chunks: Vec<BotResponseChunk>) -> Vec<BotResponseChunk> {
    let mut merged_chunks = Vec::new();

    for chunk in chunks {
        match chunk {
            BotResponseChunk::Text(text) => {
                // If the last chunk is a text chunk, merge the two
                if let Some(BotResponseChunk::Text(last_text)) =
                    merged_chunks.last_mut()
                {
                    last_text.push_str(format!("\n{}", text).as_str());
                } else {
                    // Otherwise, just add the chunk
                    merged_chunks.push(BotResponseChunk::Text(text));
                }
            }
            BotResponseChunk::Image(path) => {
                // If the last chunk is an image chunk, simply add the chunk
                merged_chunks.push(BotResponseChunk::Image(path));
            }
        }
    }

    merged_chunks
}

use std::io::Write;
use std::process::Command;

/// Takes a string, and renders it as latex to a temporary file and returns the path
/// to the file. It uses pdflatex to render the latex.
fn render_latex(latex: &str) -> String {
    // Create a file with a random name
    let filenum = rand::random::<u64>().to_string();
    let name = format!("{}.tex", filenum);
    // Open the file in the current directory
    let mut file = File::create(&name).unwrap();

    // Write the latex to the file
    writeln!(
        file,
        r"\documentclass[convert=true]{{standalone}}
\usepackage{{amsmath}}
\begin{{document}}
{}
\end{{document}}",
        latex
    )
    .unwrap();

    // Flush the file
    file.flush().unwrap();

    // Run pdflatex on the file, we have to set the cwd to the directory of the file
    let output = Command::new("xelatex")
        .arg("--shell-escape")
        .arg(name)
        .output()
        .expect("failed to execute process");

    // Check if the command failed
    if !output.status.success() {
        panic!(
            "pdflatex failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    // Get the path to the png file
    format!("{}.png", filenum)
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

    chat_log.user(&format!("{} says: {}", user_nickname, message.content))
}

async fn add_message(ctx: Context, chat_log: ChatLog, message: &Message) -> ChatLog {
    // we need to check if the id of the author is the same as the id of the bot
    if message.is_own(&ctx.cache) {
        chat_log.assistant(&message.content)
    } else {
        add_user_message(ctx, chat_log, message).await
    }
}

async fn build_chat_log(ctx: Context, messages: Vec<Message>) -> ChatLog {
    let mut chat_log = ChatLog::new();

    let prompt = include_str!("prompt.txt");

    chat_log = chat_log.system(prompt);

    for message in messages {
        chat_log = add_message(ctx.clone(), chat_log, &message).await;
    }

    chat_log
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
        if msg.author.bot {
            return;
        }

        info!("Received message: {}", msg.content);

        // See if the message is a barrier
        if msg.content == "|b|" {
            info!("Barrier received");

            // React with a checkmark
            if let Err(why) = msg.react(&ctx.http, '✅').await {
                error!("Error reacting: {:?}", why);
            }
            return;
        }

        let mut messages_to_include = Vec::new();
        messages_to_include.push(msg.clone());

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
                if message.content == "|b|" {
                    debug!("Barrier found, stopping");
                    found_barrier = true;
                    break;
                }
                messages_to_include.insert(0, message.to_owned());
            }

            // Count the number of tokens in the chat log
            let chat_log =
                build_chat_log(ctx.clone(), messages_to_include.clone()).await;

            let tokens = chat_log.count_tokens();
            if tokens > MAX_TOKENS || found_barrier {
                break;
            }
        }

        // Remove messages until we are under the limit
        while messages_to_include.len() > 1 {
            let chat_log =
                build_chat_log(ctx.clone(), messages_to_include.clone()).await;

            let tokens = chat_log.count_tokens();
            if tokens <= MAX_TOKENS {
                break;
            }

            messages_to_include.remove(0);
        }

        // Make the completion request
        let chat_log = build_chat_log(ctx.clone(), messages_to_include.clone()).await;

        debug!("Chat log: {:?}", chat_log);
        info!("Context length: {}", chat_log.count_tokens());

        // Start the "typing" indicator
        let typing = msg.channel_id.start_typing(&ctx.http);

        let completion = chat_log.complete(&self.openai).await;

        match completion {
            Ok(completion) => {
                // Turn the completion into chunks
                let chunks = chunk_response(completion.content);
                for chunk in chunks {
                    // Send the chunk
                    match chunk {
                        BotResponseChunk::Text(text) => {
                            if let Err(why) = msg.channel_id.say(&ctx.http, text).await
                            {
                                error!("Error sending message: {:?}", why);
                            }
                        }
                        BotResponseChunk::Image(image_path_str) => {
                            let image_path = Path::new(&image_path_str);
                            if let Err(why) = msg
                                .channel_id
                                .send_message(&ctx.http, |m| {
                                    m.add_file(AttachmentType::Path(image_path))
                                })
                                .await
                            {
                                error!("Error sending message: {:?}", why);
                            }
                        }
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
        println!("Client error: {:?}", why);
    }
}
