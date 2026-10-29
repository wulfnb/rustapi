# 🦀 RustAPI

A fast, file-based API testing client built in Rust, inspired by [Bruno](https://www.usebruno.com/).

![Rust](https://img.shields.io/badge/Rust-1.98.0-orange)
![License](https://img.shields.io/badge/License-GPL--3.0--or--later-blue)

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

- Rustup (the repository pins Rust 1.98.0 in `rust-toolchain.toml`)
- Cargo (comes with Rust)
- Linux build packages: install the packages in `docker/build-packages.txt` with your package manager

### Installation

```bash
# Clone the repository
git clone https://github.com/wulfnb/rustapi.git
cd rustapi

# Build and run
cargo run --release
```

### Building for Production

```bash
cargo build --release
```

The binary will be available at `target/release/rustapi`.

### Docker (browser-accessible desktop)

```bash
docker compose up --build -d
```

Open [RustAPI in your browser](http://localhost:6080/vnc.html?autoconnect=true&resize=scale).
This runs the native application on a virtual Linux display with noVNC. Collections,
environments, and history persist in the `rustapi-data` volume. The viewer has no
authentication; Compose binds it to `127.0.0.1`. Use a trusted tunnel for remote access.

To call an API running on your host, use `host.docker.internal` instead of `localhost`.
Use `docker compose down` to stop the app while retaining its data.

### Tests and CI/CD

```bash
cargo fmt --all --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
cargo build --locked
python3 scripts/desktop-smoke.py target/debug/rustapi
```

On Windows use `python` and `target/debug/rustapi.exe`. On a headless Linux machine,
run the desktop check with `xvfb-run -a` and `LIBGL_ALWAYS_SOFTWARE=1`.
Tests use temporary storage and loopback HTTP servers, with no public API dependency.

CI checks Linux, Windows x64, macOS Apple Silicon and Intel, plus the Docker desktop.
The Linux quality job enforces full Rust line/function, Python statement/branch, and
shell-launcher execution coverage; see [coverage scope and commands](docs/ci-cd.md#validation).
Pushes to `main` publish per-commit previews; tags matching `v<Cargo.toml version>`
publish versioned releases. All publishing waits for CI and packaged-app validation.
See [CI/CD and release setup](docs/ci-cd.md) for artifacts, image tags, and setup details.

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

Set `RUSTAPI_DATA_DIR` to use a different directory (Docker uses `/data`).

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

**GPL-3.0-or-later** — see [LICENSE](LICENSE) and [NOTICE](NOTICE).

Anyone may use, modify, contribute to, and sell RustAPI. Distributing a GPL-covered
derivative requires providing its corresponding source under the GPL. Private use
does not require publication. Testing another product with RustAPI does not make
that product a derivative. See [CONTRIBUTING.md](CONTRIBUTING.md) to participate.

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
