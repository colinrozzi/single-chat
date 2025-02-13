// State management
let messageCache = new Map();
let ws = null;
let reconnectAttempts = 0;
let selectedMessageId = null;
const MAX_RECONNECT_ATTEMPTS = 5;
const WEBSOCKET_URL = 'ws://localhost:{{WEBSOCKET_PORT}}/';

// UI Elements
const messageInput = document.getElementById('messageInput');
const messageArea = document.getElementById('messageArea');
const loadingOverlay = document.getElementById('messageLoading');

// Auto-resize textarea
function adjustTextareaHeight() {
    messageInput.style.height = 'auto';
    messageInput.style.height = Math.min(messageInput.scrollHeight, 200) + 'px';
}

messageInput.addEventListener('input', adjustTextareaHeight);

// WebSocket connection management
function updateConnectionStatus(status) {
    const statusElement = document.querySelector('.connection-status');
    if (!statusElement) return;
    
    statusElement.className = 'connection-status ' + status;
    
    switch(status) {
        case 'connected':
            statusElement.textContent = 'Connected';
            break;
        case 'disconnected':
            statusElement.textContent = 'Disconnected';
            break;
        case 'connecting':
            statusElement.textContent = 'Connecting...';
            break;
    }
}

function connectWebSocket() {
    updateConnectionStatus('connecting');
    
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    
    ws = new WebSocket(WEBSOCKET_URL);
    
    ws.onopen = () => {
        console.log('WebSocket connected');
        updateConnectionStatus('connected');
        reconnectAttempts = 0;
        // Request initial messages
        sendWebSocketMessage({
            type: 'get_messages'
        });
    };
    
    ws.onclose = () => {
        console.log('WebSocket disconnected');
        updateConnectionStatus('disconnected');
        if (reconnectAttempts < MAX_RECONNECT_ATTEMPTS) {
            reconnectAttempts++;
            setTimeout(connectWebSocket, 1000 * Math.min(reconnectAttempts, 30));
        }
    };
    
    ws.onerror = (error) => {
        console.error('WebSocket error:', error);
        updateConnectionStatus('disconnected');
    };
    
    ws.onmessage = (event) => {
        try {
            const data = JSON.parse(event.data);
            handleWebSocketMessage(data);
        } catch (error) {
            console.error('Error parsing WebSocket message:', error);
        }
    };
}

function sendWebSocketMessage(message) {
    if (ws && ws.readyState === WebSocket.OPEN) {
        ws.send(JSON.stringify(message));
    } else {
        console.warn('WebSocket not connected');
        updateConnectionStatus('disconnected');
    }
}

function handleWebSocketMessage(data) {
    if (data.type === 'message_update' && data.messages) {
        // Update message cache with new messages
        data.messages.forEach(msg => {
            messageCache.set(msg.id, msg);
        });
        
        // Remove any temporary messages
        for (const [id, msg] of messageCache.entries()) {
            if (id.startsWith('temp-')) {
                messageCache.delete(id);
            }
        }
        
        // Render messages without typing indicator
        renderMessages(Array.from(messageCache.values()), false);
        
        // Update head ID if present
        updateHeadId(Array.from(messageCache.values()));
    }
}

// Update head ID in title
function updateHeadId(messages) {
    const headElement = document.querySelector('.head-id');
    if (messages && messages.length > 0) {
        const lastMessage = messages[messages.length - 1];
        headElement.textContent = `Head: ${lastMessage.id.slice(0, 8)}...`;
    } else {
        headElement.textContent = 'Head: None';
    }
}

// Message handling
async function sendMessage() {
    const text = messageInput.value.trim();
    const sendButton = document.querySelector('.send-button');

    if (!text) return;

    try {
        messageInput.disabled = true;
        sendButton.disabled = true;

        // Create and show user message immediately
        const userMsg = {
            role: 'user',
            content: text,
            id: 'temp-' + Date.now(),
            parent: null
        };
        messageCache.set(userMsg.id, userMsg);
        
        // Show messages with typing indicator
        renderMessages([...messageCache.values()], true);

        // Send message to server
        sendWebSocketMessage({
            type: 'send_message',
            content: text
        });

        messageInput.value = '';
        messageInput.style.height = '2.5rem';
        messageInput.focus();
    } catch (error) {
        console.error('Error sending message:', error);
        alert('Failed to send message. Please try again.');
    } finally {
        messageInput.disabled = false;
        sendButton.disabled = false;
    }
}

// Message actions
function handleMessageClick(event) {
    const messageElement = event.target.closest('.message');
    if (!messageElement) return;

    // Don't trigger if clicking action button
    if (event.target.closest('.message-action-button')) return;

    const messageId = messageElement.dataset.id;
    
    // If clicking the same message, deselect it
    if (selectedMessageId === messageId) {
        selectedMessageId = null;
    } else {
        selectedMessageId = messageId;
    }
    renderMessages([...messageCache.values()], false);
}

function copyMessageId(messageId, button) {
    navigator.clipboard.writeText(messageId)
        .then(() => {
            // Store the original text
            const originalText = button.textContent;
            // Update the button text
            button.textContent = 'Copied!';
            // Reset after 1 second
            setTimeout(() => {
                button.textContent = originalText;
            }, 1000);
        })
        .catch(err => {
            console.error('Failed to copy message ID:', err);
            alert('Failed to copy message ID');
        });
}

function renderMessages(messages, isTyping = false) {
    // Sort messages by their sequence in the chat
    const sortedMessages = messages.sort((a, b) => {
        // If a message has a parent, it comes after that parent
        if (a.parent === b.id) return 1;
        if (b.parent === a.id) return -1;
        return 0;
    });

    if (sortedMessages.length === 0 && !isTyping) {
        messageArea.innerHTML = `
            <div class="empty-state">
                No messages yet.<br>Start the conversation!
            </div>
        `;
        return;
    }

    messageArea.innerHTML = `
        <div class="message-container">
            ${sortedMessages.map(msg => `
                <div class="message ${msg.role} ${msg.id === selectedMessageId ? 'selected' : ''}" 
                     data-id="${msg.id}">
                    ${formatMessage(msg.content)}
                    <div class="message-actions">
                        <button class="message-action-button copy-button">
                            <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor">
                                <path d="M8 5H6a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2v-1M8 5a2 2 0 002 2h2a2 2 0 002-2M8 5a2 2 0 012-2h2a2 2 0 012 2m0 0h2a2 2 0 012 2v3m2 4H10m0 0l3-3m-3 3l3 3" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
                            </svg>
                            Copy ID
                        </button>
                    </div>
                </div>
            `).join('')}
            ${isTyping ? `
                <div class="typing-indicator">
                    <span></span>
                    <span></span>
                    <span></span>
                </div>
            ` : ''}
        </div>
    `;

    // Set up event listeners for action buttons
    messageArea.querySelectorAll('.message').forEach(messageElement => {
        const messageId = messageElement.dataset.id;
        const copyButton = messageElement.querySelector('.copy-button');
        
        // Message click event
        messageElement.addEventListener('click', handleMessageClick);
        
        // Copy button click event
        if (copyButton) {
            copyButton.addEventListener('click', (event) => {
                event.stopPropagation();
                copyMessageId(messageId, copyButton);
            });
        }
    });

    messageArea.scrollTop = messageArea.scrollHeight;
}

// Message formatting
function formatMessage(content) {
    // First escape HTML and convert newlines to <br>
    let text = escapeHtml(content).replace(/\n/g, '<br>');
    
    // Format code blocks
    text = text.replace(/```([^`]+)```/g, (match, code) => `<pre><code>${code}</code></pre>`);
    
    // Format inline code
    text = text.replace(/`([^`]+)`/g, (match, code) => `<code>${code}</code>`);
    
    return text;
}

// Utility functions
function escapeHtml(unsafe) {
    return unsafe
        .replace(/&/g, "&amp;")
        .replace(/</g, "&lt;")
        .replace(/>/g, "&gt;")
        .replace(/"/g, "&quot;")
        .replace(/'/g, "&#039;");
}

// Handle clicks outside messages
document.addEventListener('click', (event) => {
    if (!event.target.closest('.message')) {
        selectedMessageId = null;
        renderMessages([...messageCache.values()], false);
    }
});

// Initialize
document.addEventListener('DOMContentLoaded', () => {
    connectWebSocket();

    // Setup message input handling
    messageInput.addEventListener('keydown', (event) => {
        if (event.key === 'Enter' && !event.shiftKey) {
            event.preventDefault();
            sendMessage();
        }
    });

    // Add global keyboard shortcut for focusing the input
    document.addEventListener('keydown', (event) => {
        // Check if user is not already typing in the input
        if (event.key === '/' && document.activeElement !== messageInput) {
            event.preventDefault(); // Prevent the '/' from being typed
            messageInput.focus();
        }
    });
});

// Handle visibility changes
document.addEventListener('visibilitychange', () => {
    if (!document.hidden && (!ws || ws.readyState !== WebSocket.OPEN)) {
        connectWebSocket();
    }
});

// Cleanup on page unload
window.addEventListener('unload', () => {
    if (ws) {
        ws.close();
    }
});