# 🦀 RustAPI

A fast, file-based API testing client built in Rust, inspired by [Bruno](https://www.usebruno.com/).

![Rust](https://img.shields.io/badge/Rust-1.70+-orange)
![License](https://img.shields.io/badge/License-MIT-blue)

## ✨ Features

- **File-Based Collections** - All your API collections are stored as JSON files, making them easy to version control with Git
- **Environment Variables** - Define variables per environment and use `{{variable}}` syntax in URLs, headers, and body
- **HTTP Methods** - Support for GET, POST, PUT, DELETE, PATCH, HEAD, OPTIONS
- **Request Body Types** - JSON, Text, Form Data, URL Encoded
- **Authentication** - Bearer Token, Basic Auth, API Key (header or query param)
- **Multiple Tabs** - Work with multiple requests simultaneously
- **Code Generation** - Generate code snippets in cURL, Python, JavaScript, and Rust
- **Request History** - Track all previously sent requests
- **Request Duplication** - Easily duplicate existing requests
- **Modern Dark UI** - Beautiful One Dark inspired theme
- **Fast & Lightweight** - Built with Rust and egui for native performance
- **Cross-Platform** - Works on Windows, macOS, and Linux

## 📁 Project Structure

```
src/
├── main.rs              # Entry point
├── app.rs               # Main application state and update loop
├── models/              # Data models
│   ├── mod.rs
│   ├── request.rs       # ApiRequest, HttpMethod, BodyType, KeyValue
│   ├── response.rs      # ApiResponse
│   ├── collection.rs    # Collection, Folder
│   ├── environment.rs   # Environment
│   ├── auth.rs          # AuthConfig, AuthType
│   └── history.rs       # RequestHistory, HistoryEntry
├── http/                # HTTP client and code generation
│   ├── mod.rs
│   ├── client.rs        # Request execution with reqwest
│   └── code_gen.rs      # Code generation (cURL, Python, JS, Rust)
├── storage/             # File-based persistence
│   ├── mod.rs
│   └── file_storage.rs  # Save/load collections, environments, history
├── ui/                  # UI components
│   ├── mod.rs
│   ├── style.rs         # Dark theme styling
│   ├── tabs.rs          # Tab management
│   ├── top_bar.rs       # Environment selector, import
│   ├── sidebar.rs       # Collections tree
│   ├── request_editor.rs# Request editing (params, headers, body)
│   ├── auth_editor.rs   # Authentication configuration
│   ├── response_viewer.rs# Response display
│   ├── modals.rs        # Modal dialogs
│   ├── code_gen_modal.rs# Code generation modal
│   └── history_panel.rs # Request history panel
└── utils/               # Utility functions
    └── mod.rs
```

## 🚀 Getting Started

### Prerequisites

- Rust 1.70 or later
- Cargo (comes with Rust)

### Installation

```bash
# Clone the repository
git clone <your-repo-url>
cd rustapi

# Build and run
cargo run --release
```

### Building for Production

```bash
cargo build --release
```

The binary will be available at `target/release/rustapi`.

## 📖 Usage

### Creating a Collection

1. Click the **➕** button next to "Collections" in the sidebar
2. Enter a name for your collection
3. Click "Create"

### Adding a Request

1. Right-click on a collection
2. Select "➕ Add Request"
3. A new request will be added and opened in a new tab

### Sending a Request

1. Select a request from the sidebar (opens in a tab)
2. Choose the HTTP method (GET, POST, etc.)
3. Enter the URL
4. Configure params, headers, body, or auth as needed
5. Click **▶ Send** or press Enter

### Using Authentication

1. Select the **Auth** tab in the request editor
2. Choose auth type:
   - **Bearer Token**: Enter your token
   - **Basic Auth**: Enter username and password
   - **API Key**: Enter key name, value, and location (header or query param)

### Generating Code

1. Configure your request
2. Click the **< >** button next to the Send button
3. Select language (cURL, Python, JavaScript, or Rust)
4. Copy the generated code

### Using Environments

1. Click the **⚙** button in the top bar
2. Create a new environment
3. Add variables (e.g., `base_url` = `https://api.example.com`)
4. Select the environment from the dropdown
5. Use `{{base_url}}` in your URLs: `{{base_url}}/users`

### Multiple Tabs

- Click on requests in the sidebar to open them in new tabs
- Click **×** on a tab to close it
- Click **➕** in the tab bar to create a new unsaved request

### Request History

1. Click **📜 History** in the top bar
2. View all previously sent requests
3. Click on any request to load it into a new tab

### Import/Export

- **Export**: Right-click a collection → Export
- **Import**: Click "📁 Import" in the top bar

## 🗂 Data Storage

RustAPI stores data in your system's data directory:

- **macOS**: `~/Library/Application Support/rustapi/`
- **Linux**: `~/.local/share/rustapi/`
- **Windows**: `%APPDATA%\rustapi\`

Structure:
```
rustapi/
├── collections/     # Your API collections (JSON files)
├── environments/    # Environment configurations
├── history.json     # Request history
└── state.json       # App state (active env, etc.)
```

## ⌨️ Keyboard Shortcuts

| Action | Shortcut |
|--------|----------|
| Send Request | `Enter` (when URL is focused) |

## 🛠 Built With

- [egui](https://github.com/emilk/egui) - Immediate mode GUI framework
- [eframe](https://github.com/emilk/egui/tree/master/crates/eframe) - Framework for egui apps
- [reqwest](https://github.com/seanmonstar/reqwest) - HTTP client
- [serde](https://github.com/serde-rs/serde) - Serialization framework
- [tokio](https://github.com/tokio-rs/tokio) - Async runtime

## 📝 License

MIT License - feel free to use this project for any purpose.

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
