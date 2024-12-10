# Skyline

A fully-featured terminal client for Bluesky, built in Rust. View your timeline, like and repost, follow users, and manage your Bluesky experience from the comfort of your terminal.

![Status: Alpha](https://img.shields.io/badge/status-alpha-orange)

## Features

- 🌈 Full-featured terminal interface with smooth scrolling and navigation
- 🖼️ Image support using Sixel protocol (compatible terminals only)
- 💬 Post, reply, like, and repost directly from your terminal
- 👤 View user profiles and manage follows
- 🔔 Real-time notifications
- 📱 Thread view support
- 🎨 Clean, intuitive interface

## Prerequisites

- Rust toolchain (rustc, cargo)
- A terminal emulator with Sixel support for images (iTerm2, Kitty, or modern xterm)
- A Bluesky account

## Installation

```bash
# Clone the repository
git clone https://github.com/Kennethprice288/skyline.git
cd skyline

# Build with release optimizations
cargo build --release

# The binary will be available at target/release/skyline
```

## Usage

Run Skyline by executing the binary:

```bash
./target/release/skyline
```

### First Time Setup

1. When you first launch Skyline, you'll be prompted to log in
2. Use the `:login username` command to start the login process
3. Enter your password when prompted
4. Once authenticated, your timeline will load automatically

### Navigation

- `j` / `k` - Scroll down/up
- `v` - View thread
- `V` - View quoted post thread
- `n` - Toggle notifications view
- `a` - View profile of post author
- `A` - View your own profile
- `ESC` - Go back/exit current view
- `q` - Quit application

### Interaction

- `l` - Like/unlike post
- `r` - Repost/unrepost post
- `f` - Follow/unfollow user
- `:post` - Create new post
- `:reply` - Reply to selected post
- `:refresh` - Refresh current view
- `:delete` - Delete your own post

### Command Mode

Enter command mode by pressing `:`. Available commands:

- `:post` - Create a new post
- `:reply` - Reply to selected post
- `:timeline` - Return to timeline
- `:notifications` - View notifications
- `:profile [handle]` - View profile (current post's author if no handle provided)
- `:refresh` - Refresh current view
- `:logout` - Log out of current session

### Post Composer

When composing posts:
- Type your message
- `Ctrl+S` to submit
- `ESC` to cancel

## Configuration

Skyline stores its configuration in `config.json` in the same directory as the binary. This file is created automatically when you first log in.

## Logging

Logs are written to `skyline.log` in the same directory as the binary.

## Building from Source

Skyline is built using Rust and Cargo. To build from source:

```bash
# Debug build
cargo build

# Release build (recommended)
cargo build --release
```

## Dependencies

- `ratatui` - Terminal user interface
- `atrium-api` - Bluesky API client
- `tokio` - Async runtime
- `crossterm` - Terminal manipulation
- Various other Rust crates for specific functionality

## Known Issues

- Image support requires a Sixel-compatible terminal
- Some newer Bluesky features may not be supported yet
- Performance may vary with large numbers of images

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request or send me an email.

## License

This project is licensed under [chosen license name] - see the [LICENSE](LICENSE) file for details

## Acknowledgments

- The Bluesky team for creating the protocol
- The Rust community for excellent libraries
