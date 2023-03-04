use std::env;

mod openai;

use openai::{ChatLog, OpenAI};

use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::prelude::*;

use log::{debug, error, info, trace, warn};

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
        .chain(fern::log_file("output.log")?)
        // remove targets different than the current one
        .filter(|metadata| metadata.target().starts_with(env!("CARGO_PKG_NAME")))
        .apply()?;
    Ok(())
}

// Constant for the maximum number of tokens in a chat log
const MAX_TOKENS: usize = 4096 - 500;

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
        if msg.content == "|barrier|" {
            info!("Barrier received");
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

            // Add them at the start of the vector
            for message in past_messages {
                // See if the message is a barrier
                if message.content == "|barrier|" {
                    debug!("Barrier found, stopping");
                    break;
                }
                messages_to_include.insert(0, message.to_owned());
            }

            // Count the number of tokens in the chat log
            let chat_log =
                build_chat_log(ctx.clone(), messages_to_include.clone()).await;

            let tokens = chat_log.count_tokens();
            if tokens > MAX_TOKENS {
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

        let completion = chat_log.complete(&self.openai).await;

        match completion {
            Ok(completion) => {
                if let Err(why) = msg
                    .channel_id
                    .say(&ctx.http, completion.content.clone())
                    .await
                {
                    error!("Error sending message: {:?}", why);
                } else {
                    info!("Sent message: {}", completion.content);
                }
            }
            Err(why) => {
                error!("Error completing chat: {:?}", why);
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
