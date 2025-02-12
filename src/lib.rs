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
use std::collections::HashMap;

// Message struct changes - making id optional
#[derive(Serialize, Deserialize, Debug, Clone)]
struct Message {
    role: String,
    content: String,
    parent: Option<String>,
    id: Option<String>, // Now optional
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
        Self {
            role,
            content,
            parent,
            id: None, // No ID until stored
        }
    }

    // Helper to create a message with ID (for after storage)
    fn with_id(mut self, id: String) -> Self {
        self.id = Some(id);
        self
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct State {
    chat: Chat,
    api_key: String,
    connected_clients: HashMap<String, bool>,
    key_value_actor: String, // Add this field
}

// Import the Request/Action types - we'll need to define these since we can't import from key-value actor
#[derive(Serialize, Deserialize, Debug)]
struct Request {
    _type: String,
    data: Action,
}

#[derive(Serialize, Deserialize, Debug)]
enum Action {
    Get(String),
    Put(Vec<u8>),
    All(()),
}

impl State {
    fn save_message(&self, msg: &Message) -> Result<String, Box<dyn std::error::Error>> {
        let request = Request {
            _type: "request".to_string(),
            data: Action::Put(serde_json::to_vec(&msg)?),
        };

        let request_bytes = serde_json::to_vec(&request)?;
        let response_bytes = bindings::ntwk::theater::message_server_host::request(
            &self.key_value_actor,
            &request_bytes,
        )?;

        let response: Value = serde_json::from_slice(&response_bytes)?;
        if response["status"].as_str() == Some("ok") {
            response["key"]
                .as_str()
                .map(|s| s.to_string())
                .ok_or("No key in response".into())
        } else {
            Err("Failed to save message".into())
        }
    }

    fn load_message(&self, id: &str) -> Result<Message, Box<dyn std::error::Error>> {
        let request = Request {
            _type: "request".to_string(),
            data: Action::Get(id.to_string()),
        };

        let request_bytes = serde_json::to_vec(&request)?;
        let response_bytes = bindings::ntwk::theater::message_server_host::request(
            &self.key_value_actor,
            &request_bytes,
        )?;

        let response: Value = serde_json::from_slice(&response_bytes)?;
        if response["status"].as_str() == Some("ok") {
            if let Some(value) = response.get("value") {
                // The value should be an array of bytes that we can directly deserialize
                let bytes = value
                    .as_array()
                    .ok_or("Expected byte array")?
                    .iter()
                    .map(|v| v.as_u64().unwrap_or(0) as u8)
                    .collect::<Vec<u8>>();
                let mut msg: Message = serde_json::from_slice(&bytes)?;
                msg.id = Some(id.to_string());
                return Ok(msg);
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

    fn update_head(&mut self, message_id: String) -> Result<(), Box<dyn std::error::Error>> {
        self.chat.head = Some(message_id);
        Ok(())
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

#[derive(Serialize, Deserialize, Debug)]
struct InitData {
    key_value_actor: String,
}

struct Component;

impl ActorGuest for Component {
    fn init(data: Option<Vec<u8>>) -> Vec<u8> {
        log("Initializing single chat actor");
        let data = data.unwrap();
        log(&format!("Data: {:?}", data));

        let init_data: InitData = serde_json::from_slice(&data).unwrap();

        log(&format!("Key value actor: {}", init_data.key_value_actor));

        // Read API key
        let api_key = read_file("api-key.txt").unwrap();
        let api_key = String::from_utf8(api_key).unwrap().trim().to_string();

        // Load or create chat
        let chat = State::load_chat().unwrap_or_else(|_| Chat { head: None });

        let initial_state = State {
            chat,
            api_key,
            connected_clients: HashMap::new(),
            key_value_actor: init_data.key_value_actor,
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
                                    // Create initial user message without ID
                                    let user_msg = Message::new(
                                        "user".to_string(),
                                        content.to_string(),
                                        current_state.chat.head.clone(),
                                    );

                                    // Save message and get its ID
                                    if let Ok(msg_id) = current_state.save_message(&user_msg) {
                                        if current_state.update_head(msg_id.clone()).is_ok() {
                                            // Create final message with ID
                                            let user_msg_with_id = user_msg.with_id(msg_id);

                                            // Get message history for context
                                            if let Ok(messages) =
                                                current_state.get_message_history()
                                            {
                                                // Generate AI response
                                                if let Ok(ai_response) =
                                                    current_state.generate_response(messages)
                                                {
                                                    let ai_msg = Message::new(
                                                        "assistant".to_string(),
                                                        ai_response,
                                                        user_msg_with_id.id.clone(),
                                                    );

                                                    // Save AI message and get its ID
                                                    if let Ok(ai_msg_id) =
                                                        current_state.save_message(&ai_msg)
                                                    {
                                                        if current_state
                                                            .update_head(ai_msg_id.clone())
                                                            .is_ok()
                                                        {
                                                            let ai_msg_with_id =
                                                                ai_msg.with_id(ai_msg_id);

                                                            // Send response with both messages
                                                            return (
                                                                serde_json::to_vec(&current_state).unwrap(),
                                                                WebsocketResponse {
                                                                    messages: vec![WebsocketMessage {
                                                                        ty: MessageType::Text,
                                                                        text: Some(
                                                                            serde_json::json!({
                                                                                "type": "message_update",
                                                                                "messages": [user_msg_with_id, ai_msg_with_id]
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
