# Single Chat Actor

A self-contained WebAssembly actor that provides a complete LLM chat interface using Claude. This actor combines a chat interface, state management, and LLM integration into a single component.

## Features

- 🌐 Complete web interface for chat interactions
  - Modern, responsive design
  - Code block formatting with syntax highlighting
  - Message actions (copy message ID, etc.)
  - Real-time typing indicators
  - Connection status display
- 💾 Persistent chat history with linked message structure
- 🤖 Integration with Anthropic's Claude API (using claude-3-5-sonnet-20241022)
- 🔄 Real-time updates via WebSocket
- 📱 Responsive design that works on all devices

## Quick Start

1. Clone the repository
2. Create an `api-key.txt` file in the assets directory with your Anthropic API key
3. Build the actor:
```bash
cargo build --release
```

4. Run using the Theater runtime:
```bash
theater run actor.toml
```

5. Open `http://localhost:8080` in your browser

## Project Structure

```
single-chat/
├── Cargo.toml              # Project configuration
├── actor.toml             # Actor manifest
├── src/
│   ├── lib.rs            # Actor implementation
│   └── bindings.rs       # Generated bindings
├── assets/               # Web assets
│   ├── index.html       # Main HTML file
│   ├── styles.css       # CSS styles
│   ├── chat.js         # Frontend JavaScript
│   └── api-key.txt     # Your Anthropic API key
└── README.md            # This file
```

## Frontend Features

### Interface
- Clean, modern chat interface with message bubbles
- Real-time message updates
- Code block formatting with syntax highlighting
- Message actions (accessible by clicking messages)
- Auto-expanding text input
- Connection status indicator
- Keyboard shortcuts (/ to focus input, Shift+Enter for new line)

### User Experience
- Responsive design that works on all screen sizes
- Visual feedback for message states
- Clear error handling
- Persistent WebSocket connection with automatic reconnection
- Loading states and typing indicators

## Backend Features

### Core Functionality
- Persistent chat history using a key-value store
- WebSocket for real-time updates
- RESTful API endpoints
- Claude 3.5 Sonnet integration
- State management with verification
- Linked message structure (each message references its parent)

### Message Handling
- Asynchronous message processing
- Error handling and retry logic
- Message ID generation and tracking
- Parent-child message relationships

## API Endpoints

- `GET /` - Serves the web interface
- `GET /api/messages` - Get all messages in the chat
- `WS /` - WebSocket endpoint for real-time updates

## WebSocket Events

- `get_messages` - Request all messages
- `send_message` - Send a new message
- `message_update` - Receive message updates

## Configuration

The actor can be configured via `actor.toml`:

```toml
name = "single-chat"
version = "0.1.0"
description = "Single chat actor with Claude integration"
component_path = "target/wasm32-wasi/release/single_chat.wasm"

[interface]
implements = "ntwk:theater/actor"
requires = []

[[handlers]]
type = "http-server"
config = { port = 8080 }

[[handlers]]
type = "websocket-server"
config = { port = 8081 }
```

## Development

### Prerequisites

- Rust (latest stable)
- wasm32-wasi target: `rustup target add wasm32-wasi`
- Theater runtime

### Building

```bash
# Build the actor
cargo build --release

# Run the actor
theater run actor.toml
```

### Testing

```bash
# Run tests
cargo test
```

## Architecture

The actor combines several components into a single WebAssembly module:

1. **HTTP Server**: Serves the web interface and handles API requests
2. **WebSocket Server**: Provides real-time updates and message handling
3. **State Management**: Handles chat history using a linked message structure
4. **LLM Integration**: Communicates with Claude 3.5 Sonnet for message generation
5. **Message Actions**: Provides interaction capabilities for individual messages

### Data Flow

1. User interacts with web interface
2. WebSocket sends message to actor
3. Actor processes message and updates state
4. Actor sends message to Claude
5. Response is received and sent back to client
6. State is updated and persisted

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

MIT License - feel free to use this code in your own projects.

## Credits

This actor is part of the Theater actor system and uses several great technologies:

- [Theater](https://github.com/ntwk/theater) - Actor system
- [Claude](https://anthropic.com) - Language model
- Rust, WebAssembly, and various web technologies

## Security

The actor includes several security features:

- Input validation
- State verification
- Secure WebSocket connections
- API key protection
- Message ID validation

Please note that you should keep your `api-key.txt` secure and never commit it to version control.
