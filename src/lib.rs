mod bindings;

use bindings::exports::ntwk::theater::actor::Guest as ActorGuest;
use bindings::exports::ntwk::theater::http_server::Guest as HttpGuest;
use bindings::exports::ntwk::theater::http_server::{
    HttpRequest as ServerHttpRequest, HttpResponse,
};
use bindings::exports::ntwk::theater::message_server_client::Guest as MessageServerClientGuest;
use bindings::exports::ntwk::theater::websocket_server::Guest as WebSocketGuest;
use bindings::exports::ntwk::theater::websocket_server::{
    MessageType, WebsocketMessage, WebsocketResponse,
};
use bindings::ntwk::theater::filesystem::{path_exists, read_file};
use bindings::ntwk::theater::http_client::{send_http, HttpRequest};
use bindings::ntwk::theater::runtime::log;
use bindings::ntwk::theater::types::Json;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha1::{Digest, Sha1};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Message {
    role: String,
    content: String,
    parent: Option<String>,
    id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct AnthropicMessage {
    role: String,
    content: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Chat {
    head: Option<String>,
}

impl Message {
    fn new(role: String, content: String, parent: Option<String>) -> Self {
        let temp_msg = Self {
            role,
            content,
            parent,
            id: String::new(),
        };

        let mut hasher = Sha1::new();
        let temp_json = serde_json::to_string(&temp_msg).unwrap();
        hasher.update(temp_json.as_bytes());
        let id = format!("{:x}", hasher.finalize());

        Self { id, ..temp_msg }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct State {
    chat: Chat,
    api_key: String,
    connected_clients: HashMap<String, bool>,
    key_value_actor: String, // Add this field
}

impl State {
    fn save_message(&self, msg: &Message) -> Result<(), Box<dyn std::error::Error>> {
        let request = Request {
            _type: "request".to_string(),
            data: Action::Put(serde_json::to_vec(&msg)?),
        };

        let request_bytes = serde_json::to_vec(&request)?;
        if let Ok(response_bytes) = bindings::ntwk::theater::message_server_client::request(
            &self.key_value_actor,
            &request_bytes,
        ) {
            let response: Value = serde_json::from_slice(&response_bytes)?;
            if response["status"] == "ok" {
                return Ok(());
            }
        }
        Err("Failed to save message".into())
    }

    fn load_message(&self, id: &str) -> Result<Message, Box<dyn std::error::Error>> {
        let request = Request {
            _type: "request".to_string(),
            data: Action::Get(id.to_string()),
        };

        let request_bytes = serde_json::to_vec(&request)?;
        if let Ok(response_bytes) = bindings::ntwk::theater::message_server_client::request(
            &self.key_value_actor,
            &request_bytes,
        ) {
            let response: Value = serde_json::from_slice(&response_bytes)?;
            if response["status"] == "ok" {
                if let Some(value) = response["value"].as_array() {
                    return Ok(serde_json::from_slice(&value)?);
                }
            }
        }
        Err("Failed to load message".into())
    }

    fn get_message_history(&self) -> Result<Vec<Message>, Box<dyn std::error::Error>> {
        let mut messages = Vec::new();
        let mut current_id = self.chat.head.clone();

        while let Some(id) = current_id {
            let msg = self.load_message(&id)?;
            messages.push(msg.clone());
            current_id = msg.parent.clone();
        }

        messages.reverse(); // Oldest first
        Ok(messages)
    }

    fn load_chat() -> Result<Chat, Box<dyn std::error::Error>> {
        let path = "data/chats/chat.json";
        if path_exists(path).unwrap_or(false) {
            let content = read_file(path).unwrap();
            Ok(serde_json::from_slice(&content)?)
        } else {
            Ok(Chat { head: None })
        }
    }

    fn update_head(&mut self, message_id: String) -> () {
        self.chat.head = Some(message_id)
    }

    fn generate_response(
        &self,
        messages: Vec<Message>,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let anthropic_messages: Vec<AnthropicMessage> = messages
            .iter()
            .map(|msg| AnthropicMessage {
                role: msg.role.clone(),
                content: msg.content.clone(),
            })
            .collect();

        let request = HttpRequest {
            method: "POST".to_string(),
            uri: "https://api.anthropic.com/v1/messages".to_string(),
            headers: vec![
                ("Content-Type".to_string(), "application/json".to_string()),
                ("x-api-key".to_string(), self.api_key.clone()),
                ("anthropic-version".to_string(), "2023-06-01".to_string()),
            ],
            body: Some(
                serde_json::to_vec(&json!({
                    "model": "claude-3-5-sonnet-20241022",
                    "max_tokens": 1024,
                    "messages": anthropic_messages,
                }))
                .unwrap(),
            ),
        };

        let http_response = send_http(&request);

        if let Some(body) = http_response.body {
            if let Ok(response_data) = serde_json::from_slice::<Value>(&body) {
                if let Some(text) = response_data["content"][0]["text"].as_str() {
                    return Ok(text.to_string());
                }
            }
        }

        Err("Failed to generate response".into())
    }
}

struct Component;

impl ActorGuest for Component {
    fn init(_data: Vec<u8>) -> Vec<u8> {
        log("Initializing single chat actor");

        // Read API key
        let api_key = read_file("api-key.txt").unwrap();
        let api_key = String::from_utf8(api_key).unwrap().trim().to_string();

        // Load or create chat
        let chat = State::load_chat().unwrap_or_else(|_| Chat { head: None });

        let initial_state = State {
            chat,
            api_key,
            connected_clients: HashMap::new(),
            key_value_actor: "key-value-store".to_string(),
        };

        serde_json::to_vec(&initial_state).unwrap()
    }
}

impl HttpGuest for Component {
    fn handle_request(req: ServerHttpRequest, state: Json) -> (HttpResponse, Json) {
        log(&format!("Handling HTTP request for: {}", req.uri));

        match (req.method.as_str(), req.uri.as_str()) {
            ("GET", "/") | ("GET", "/index.html") => {
                let content = read_file("index.html").unwrap();
                (
                    HttpResponse {
                        status: 200,
                        headers: vec![("Content-Type".to_string(), "text/html".to_string())],
                        body: Some(content),
                    },
                    state,
                )
            }
            ("GET", "/styles.css") => {
                let content = read_file("styles.css").unwrap();
                (
                    HttpResponse {
                        status: 200,
                        headers: vec![("Content-Type".to_string(), "text/css".to_string())],
                        body: Some(content),
                    },
                    state,
                )
            }
            ("GET", "/chat.js") => {
                let content = read_file("chat.js").unwrap();
                (
                    HttpResponse {
                        status: 200,
                        headers: vec![(
                            "Content-Type".to_string(),
                            "application/javascript".to_string(),
                        )],
                        body: Some(content),
                    },
                    state,
                )
            }

            ("GET", "/api/messages") => {
                let current_state: State = serde_json::from_slice(&state).unwrap();
                match current_state.get_message_history() {
                    Ok(messages) => (
                        HttpResponse {
                            status: 200,
                            headers: vec![(
                                "Content-Type".to_string(),
                                "application/json".to_string(),
                            )],
                            body: Some(
                                serde_json::to_vec(&json!({
                                    "status": "success",
                                    "messages": messages
                                }))
                                .unwrap(),
                            ),
                        },
                        state,
                    ),
                    Err(_) => (
                        HttpResponse {
                            status: 500,
                            headers: vec![],
                            body: Some(b"Failed to load messages".to_vec()),
                        },
                        state,
                    ),
                }
            }

            // Default 404 response
            _ => (
                HttpResponse {
                    status: 404,
                    headers: vec![],
                    body: Some(b"Not Found".to_vec()),
                },
                state,
            ),
        }
    }
}

impl WebSocketGuest for Component {
    fn handle_message(msg: WebsocketMessage, state: Json) -> (Json, WebsocketResponse) {
        let mut current_state: State = serde_json::from_slice(&state).unwrap();

        match msg.ty {
            MessageType::Text => {
                if let Some(text) = msg.text {
                    if let Ok(command) = serde_json::from_str::<Value>(&text) {
                        match command["type"].as_str() {
                            Some("send_message") => {
                                if let Some(content) = command["content"].as_str() {
                                    // Create user message
                                    let user_msg = Message::new(
                                        "user".to_string(),
                                        content.to_string(),
                                        current_state.chat.head.clone(),
                                    );

                                    if current_state.save_message(&user_msg).is_ok() {
                                        current_state.update_head(user_msg.id.clone());
                                        // Get message history for context
                                        if let Ok(messages) = current_state.get_message_history() {
                                            // Generate AI response
                                            if let Ok(ai_response) =
                                                current_state.generate_response(messages)
                                            {
                                                let ai_msg = Message::new(
                                                    "assistant".to_string(),
                                                    ai_response,
                                                    Some(user_msg.id.clone()),
                                                );

                                                if current_state.save_message(&ai_msg).is_ok() {
                                                    current_state.update_head(ai_msg.id.clone());

                                                    // Send response with both messages
                                                    return (
                                                                serde_json::to_vec(&current_state).unwrap(),
                                                                WebsocketResponse {
                                                                    messages: vec![WebsocketMessage {
                                                                        ty: MessageType::Text,
                                                                        text: Some(
                                                                            serde_json::json!({
                                                                                "type": "message_update",
                                                                                "messages": [user_msg, ai_msg]
                                                                            })
                                                                            .to_string(),
                                                                        ),
                                                                        data: None,
                                                                    }],
                                                                },
                                                            );
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            Some("get_messages") => {
                                if let Ok(messages) = current_state.get_message_history() {
                                    return (
                                        serde_json::to_vec(&current_state).unwrap(),
                                        WebsocketResponse {
                                            messages: vec![WebsocketMessage {
                                                ty: MessageType::Text,
                                                text: Some(
                                                    serde_json::json!({
                                                        "type": "message_update",
                                                        "messages": messages
                                                    })
                                                    .to_string(),
                                                ),
                                                data: None,
                                            }],
                                        },
                                    );
                                }
                            }
                            _ => {
                                log("Unknown command type received");
                            }
                        }
                    }
                }
            }
            _ => {}
        }

        (
            serde_json::to_vec(&current_state).unwrap(),
            WebsocketResponse { messages: vec![] },
        )
    }
}

impl MessageServerClientGuest for Component {
    fn handle_send(msg: Vec<u8>, state: Json) -> Json {
        log("Handling message server client send");
        let msg_str = String::from_utf8(msg).unwrap();
        log(&msg_str);
        state
    }

    fn handle_request(msg: Vec<u8>, state: Json) -> (Vec<u8>, Json) {
        log("Handling message server client request");
        let msg_str = String::from_utf8(msg).unwrap();
        log(&msg_str);
        (vec![], state)
    }
}

bindings::export!(Component with_types_in bindings);
