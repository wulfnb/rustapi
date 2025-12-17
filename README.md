# 🦀 RustAPI

A fast, file-based API testing client built in Rust, inspired by [Bruno](https://www.usebruno.com/).

![Rust](https://img.shields.io/badge/Rust-1.70+-orange)
![License](https://img.shields.io/badge/License-MIT-blue)

## ✨ Features

- **File-Based Collections** - All your API collections are stored as JSON files, making them easy to version control with Git
- **Environment Variables** - Define variables per environment and use `{{variable}}` syntax in URLs, headers, and body
- **HTTP Methods** - Support for GET, POST, PUT, DELETE, PATCH, HEAD, OPTIONS
- **Request Body Types** - JSON, Text, Form Data, URL Encoded
- **Modern Dark UI** - Beautiful One Dark inspired theme
- **Fast & Lightweight** - Built with Rust and egui for native performance
- **Cross-Platform** - Works on Windows, macOS, and Linux

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
2. Select "Add Request"
3. A new request will be added to the collection

### Sending a Request

1. Select a request from the sidebar
2. Choose the HTTP method (GET, POST, etc.)
3. Enter the URL
4. Add headers, query params, or body as needed
5. Click **Send** or press Enter

### Using Environments

1. Click the **⚙** button in the top bar to manage environments
2. Create a new environment
3. Add variables (e.g., `base_url` = `https://api.example.com`)
4. Select the environment from the dropdown
5. Use `{{base_url}}` in your URLs: `{{base_url}}/users`

### Import/Export

- **Export**: Right-click a collection → Export
- **Import**: Click the "Import" button in the top bar

## 🗂 Data Storage

RustAPI stores data in your system's data directory:

- **macOS**: `~/Library/Application Support/rustapi/`
- **Linux**: `~/.local/share/rustapi/`
- **Windows**: `%APPDATA%\rustapi\`

Structure:
```
rustapi/
├── collections/     # Your API collections
├── environments/    # Environment configurations
└── state.json       # App state (window size, active env, etc.)
```

## ⌨️ Keyboard Shortcuts

| Action | Shortcut |
|--------|----------|
| Send Request | `Enter` (when URL is focused) |
| Save Request | Click "Save" button |

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


