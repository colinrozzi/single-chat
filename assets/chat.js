// Title editing
function startEditingTitle() {
    const titleElement = document.querySelector('.title');
    const currentTitle = titleElement.textContent;
    titleElement.contentEditable = true;
    titleElement.classList.add('editing');
    titleElement.focus();

    // Save on enter or blur
    titleElement.addEventListener('keydown', function titleKeydown(e) {
        if (e.key === 'Enter') {
            e.preventDefault();
            titleElement.blur();
        }
        if (e.key === 'Escape') {
            titleElement.textContent = currentTitle;
            titleElement.blur();
        }
    });

    titleElement.addEventListener('blur', function titleBlur() {
        const newTitle = titleElement.textContent.trim();
        if (newTitle && newTitle !== currentTitle) {
            sendWebSocketMessage({
                type: 'update_title',
                title: newTitle
            });
        } else {
            titleElement.textContent = currentTitle;
        }
        titleElement.contentEditable = false;
        titleElement.classList.remove('editing');
    }, { once: true });
}

// State management
let messageCache = new Map();
let ws = null;
let reconnectAttempts = 0;
const MAX_RECONNECT_ATTEMPTS = 5;
let isTyping = false;

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
    const wsUrl = 'ws://localhost:8082/';
    
    ws = new WebSocket(wsUrl);
    
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
            // Try to parse state from data
            if (data.state) {
                const state = JSON.parse(data.state);
                updateTitle(state);
            }
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

function updateTitle(state) {
    const titleElement = document.querySelector('.title');
    if (titleElement && state.title) {
        titleElement.textContent = state.title;
    }
}

function handleWebSocketMessage(data) {
    // Handle full message history
    if (data.type === 'message_update' && data.messages) {
        // Hide both loading and typing indicators
        hideTypingIndicator();
        hideLoading();
        
        // Add new messages to cache
        data.messages.forEach(msg => {
            messageCache.set(msg.id, msg);
        });
        // Render all messages from cache
        renderMessages(Array.from(messageCache.values()));
        updateHeadId(Array.from(messageCache.values()));
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

        // Show typing indicator before sending message
        showTypingIndicator();

        sendWebSocketMessage({
            type: 'send_message',
            content: text
        });

        messageInput.value = '';
        messageInput.style.height = '2.5rem';
        messageInput.focus();
    } catch (error) {
        console.error('Error sending message:', error);
        hideTypingIndicator();
        alert('Failed to send message. Please try again.');
    } finally {
        messageInput.disabled = false;
        sendButton.disabled = false;
    }
}

function renderMessages(messages) {
    // Sort messages by their sequence in the chat
    const sortedMessages = messages.sort((a, b) => {
        // If a message has a parent, it comes after that parent
        if (a.parent === b.id) return 1;
        if (b.parent === a.id) return -1;
        return 0;
    });

    if (sortedMessages.length === 0) {
        messageArea.innerHTML = `
            <div class="empty-state">
                No messages yet.<br>Start the conversation!
            </div>
        `;
        return;
    }

    // Add messages
    messageArea.innerHTML = `
        <div class="message-container">
            ${sortedMessages.map(msg => `
                <div class="message ${msg.role}" data-id="${msg.id}">
                    ${formatMessage(msg.content)}
                </div>
            `).join('')}
            <div class="typing-indicator">
                <span></span>
                <span></span>
                <span></span>
            </div>
        </div>
    `;

    messageArea.scrollTop = messageArea.scrollHeight;
}

// Typing indicator management
function showTypingIndicator() {
    if (!isTyping) {
        isTyping = true;
        const typingIndicator = messageArea.querySelector('.typing-indicator');
        if (typingIndicator) {
            typingIndicator.classList.add('visible');
            messageArea.scrollTop = messageArea.scrollHeight;
        }
    }
}

function hideTypingIndicator() {
    isTyping = false;
    const typingIndicator = messageArea.querySelector('.typing-indicator');
    if (typingIndicator) {
        typingIndicator.classList.remove('visible');
    }
}

// Loading state management
function showLoading() {
    loadingOverlay.classList.add('show');
}

function hideLoading() {
    loadingOverlay.classList.remove('show');
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

    // Show initial loading state
    showLoading();
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