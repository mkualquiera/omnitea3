use std::env;

mod openai;

use openai::{ChatLog, OpenAI};

use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::prelude::*;

struct Handler {
    openai: OpenAI,
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

        let prompt = include_str!("prompt.txt");

        let mut chat_log = ChatLog::new().system(prompt);

        // Read channel messages to build chat log
        let mut messages = msg
            .channel_id
            .messages(&ctx.http, |retriever| retriever.before(msg.id).limit(10))
            .await
            .unwrap();

        messages.reverse();

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

        async fn add_message(
            ctx: Context,
            chat_log: ChatLog,
            message: &Message,
        ) -> ChatLog {
            // we need to check if the id of the author is the same as the id of the bot
            if message.is_own(&ctx.cache) {
                chat_log.assistant(&message.content)
            } else {
                add_user_message(ctx, chat_log, message).await
            }
        }

        for message in messages {
            if message.author.bot {
                chat_log = chat_log.assistant(&message.content);
            } else {
                chat_log = add_user_message(ctx.clone(), chat_log, &message).await;
            }
        }

        // Add user message
        chat_log = add_user_message(ctx.clone(), chat_log, &msg).await;

        println!("{:?}", chat_log);

        let completion = chat_log.complete(&self.openai).await;

        match completion {
            Ok(completion) => {
                if let Err(why) =
                    msg.channel_id.say(&ctx.http, completion.content).await
                {
                    println!("Error sending message: {:?}", why);
                }
            }
            Err(why) => {
                println!("Error completing chat: {:?}", why);
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
