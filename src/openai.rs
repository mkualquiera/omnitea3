use serde::{Deserialize, Serialize};

/// Roles that can be used in a chat log
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum ChatRole {
    /// The system, used for the initial prompt and maybe other things
    #[serde(rename = "system")]
    System,
    /// The user, used for the user's input
    #[serde(rename = "user")]
    User,
    /// The assistant, used for the assistant's response
    #[serde(rename = "assistant")]
    Assistant,
}

/// A single entry in a chat log
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ChatEntry {
    /// The role of the entry
    pub role: ChatRole,
    /// The text of the entry
    pub content: String,
}

/// A chat completion request
#[derive(Serialize, Deserialize, Debug)]
struct ChatCompletionRequest {
    /// The model used for the completion
    model: String,
    /// The chat log
    messages: ChatLog,
}

impl ChatCompletionRequest {
    /// Create a new chat completion request
    fn new(model: &str, messages: ChatLog) -> ChatCompletionRequest {
        ChatCompletionRequest {
            model: model.to_string(),
            messages,
        }
    }
}

impl From<ChatLog> for ChatCompletionRequest {
    /// Create a new chat completion request from a chat log
    fn from(log: ChatLog) -> ChatCompletionRequest {
        ChatCompletionRequest::new("gpt-3.5-turbo", log)
    }
}

/// A chat log, which is a list of chat entries
#[derive(Serialize, Deserialize, Debug)]
pub struct ChatLog(Vec<ChatEntry>);

/// Chat completion choice
#[derive(Serialize, Deserialize, Debug)]
pub struct ChatCompletionChoice {
    /// The text of the choice
    pub index: usize,
    /// The message of the choice
    pub message: ChatEntry,
}

/// A completion usage information
#[derive(Serialize, Deserialize, Debug)]
pub struct CompletionUsage {
    /// The tokens in the prompt
    prompt_tokens: usize,
    /// The tokens in the completion
    completion_tokens: usize,
    /// The tokens in the total
    total_tokens: usize,
}

/// A chat completion response
#[derive(Serialize, Deserialize, Debug)]
pub struct ChatCompletionResponse {
    /// The completion id
    id: String,
    /// The completion object
    object: String,
    /// The completion creation time
    created: usize,
    /// The completion choices
    pub choices: Vec<ChatCompletionChoice>,
    /// The completion usage
    pub usage: CompletionUsage,
}

/// OpenAI api clients
pub struct OpenAI {
    /// HTTP client
    client: reqwest::Client,
    /// OpenAI api key
    api_key: String,
}

impl ChatLog {
    /// Create a new chat log
    pub fn new() -> ChatLog {
        ChatLog(Vec::new())
    }

    /// Add a new entry to the chat log
    pub fn add(mut self, role: ChatRole, content: &str) -> ChatLog {
        self.0.push(ChatEntry {
            role,
            content: content.to_string(),
        });
        self
    }

    /// Add a new system entry to the chat log
    pub fn system(self, content: &str) -> ChatLog {
        self.add(ChatRole::System, content)
    }

    /// Add a new user entry to the chat log
    pub fn user(self, content: &str) -> ChatLog {
        self.add(ChatRole::User, content)
    }

    /// Add a new assistant entry to the chat log
    pub fn assistant(self, content: &str) -> ChatLog {
        self.add(ChatRole::Assistant, content)
    }

    /// Complete the chat log
    pub async fn complete(self, client: &OpenAI) -> Result<ChatEntry, String> {
        client.complete_chat(self).await.map_or_else(
            |e| Err(e.to_string()),
            |response| {
                response.choices.get(0).map_or_else(
                    || Err("No choices".to_string()),
                    |choice| Ok(choice.message.clone()),
                )
            },
        )
    }
}

impl OpenAI {
    /// Create a new OpenAI client
    pub fn new(api_key: String) -> OpenAI {
        OpenAI {
            client: reqwest::Client::new(),
            api_key,
        }
    }

    /// Complete a chat
    pub async fn complete_chat(
        &self,
        chat: ChatLog,
    ) -> Result<ChatCompletionResponse, reqwest::Error> {
        let request = ChatCompletionRequest::from(chat);

        // Make post request to OpenAI
        self.client
            .post("https://api.openai.com/v1/chat/completions")
            .bearer_auth(self.api_key.clone())
            .json(&request)
            .send()
            .await?
            .json::<ChatCompletionResponse>()
            .await
    }
}
