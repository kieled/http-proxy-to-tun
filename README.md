# http-tun

![image (1)](https://github.com/user-attachments/assets/067e07f4-5a36-4bf7-bd78-6b7cc2b8528e)

A desktop application that routes your entire system's traffic through an HTTP proxy using a TUN (virtual network) interface. Built with Rust and Tauri.

> **Note**: This application currently only supports **Linux**.

## What Does It Do?

When you run http-tun, it creates a virtual network interface that captures all your system's network traffic and forwards it through an HTTP proxy of your choice. This is useful for:

- Routing all traffic through a corporate proxy
- Privacy and anonymity setups
- Network debugging and monitoring

## Prerequisites

Before you begin, make sure you have these installed on your system:

### 1. Rust (Programming Language)

Install Rust using rustup (the official installer):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

After installation, restart your terminal or run:

```bash
source ~/.cargo/env
```

Verify the installation:

```bash
rustc --version
# Should show something like: rustc 1.XX.X
```

### 2. Node.js and bun

Install Node.js 18+ from [nodejs.org](https://nodejs.org/) or using your package manager.

Then install bun:

```bash
curl -fsSL https://bun.sh/install | bash
```

Verify installation:

```bash
node --version  # Should be 18.x or higher
bun --version   # Should show version number
```

### 3. System Dependencies (Linux)

Install the required system libraries:

**Debian/Ubuntu:**

```bash
sudo apt update
sudo apt install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    libgtk-3-dev \
    libwebkit2gtk-4.1-dev \
    librsvg2-dev \
    libappindicator3-dev \
    libmnl-dev \
    libnftnl-dev
```

**Fedora:**

```bash
sudo dnf install -y \
    gcc \
    pkg-config \
    openssl-devel \
    gtk3-devel \
    webkit2gtk4.1-devel \
    librsvg2-devel \
    libappindicator-gtk3-devel \
    libmnl-devel \
    libnftnl-devel
```

**Arch Linux:**

```bash
sudo pacman -S --needed \
    base-devel \
    openssl \
    gtk3 \
    webkit2gtk-4.1 \
    librsvg \
    libappindicator-gtk3 \
    libmnl \
    libnftnl
```

### 4. Tauri CLI

Install the Tauri command-line tools:

```bash
cargo install tauri-cli
```

## Installation & Setup

### Step 1: Clone the Repository

```bash
git clone https://github.com/YOUR_USERNAME/http-tun.git
cd http-tun
```

### Step 2: Install Frontend Dependencies

Navigate to the UI directory and install packages:

```bash
cd ui
bun install
cd ..
```

## Development

### Running in Development Mode

Development mode enables hot-reloading so you can see changes instantly.

**Terminal 1** - Start the frontend dev server:

```bash
cd ui
bun run dev
```

**Terminal 2** - Start the Tauri app:

```bash
cd crates/tauri-app
cargo tauri dev
```

The app will open automatically. Any changes you make to the frontend code will reload instantly.

> **Note**: The app requires root privileges to create TUN interfaces. You may be prompted for your password.

## Building for Production

### Build the Application

This creates an optimized, release-ready version of the app:

```bash
# Build the frontend first
cd ui
bun run build
cd ..

# Build the Tauri app
cd crates/tauri-app
cargo tauri build
```

The built application will be located in:

```
crates/tauri-app/target/release/bundle/
```

You'll find different formats depending on your needs:

- `deb/` - Debian package (.deb) for Ubuntu/Debian
- `appimage/` - Portable AppImage that runs on any Linux
- `rpm/` - RPM package for Fedora/RHEL

### Install the Built Application

**Debian/Ubuntu (.deb):**

```bash
sudo dpkg -i crates/tauri-app/target/release/bundle/deb/http-tunnel_*.deb
```

**AppImage (portable, no install needed):**

```bash
chmod +x crates/tauri-app/target/release/bundle/appimage/http-tunnel_*.AppImage
./crates/tauri-app/target/release/bundle/appimage/http-tunnel_*.AppImage
```

**Fedora (.rpm):**

```bash
sudo rpm -i crates/tauri-app/target/release/bundle/rpm/http-tunnel-*.rpm
```

## Running the Application

After installation, you can run the app from your application menu or from the terminal:

```bash
http-tun-desktop
```

> **Important**: The app requires root/sudo privileges to work with network interfaces.

## Project Structure

```
http-tun/
├── ui/                     # React frontend (TypeScript)
├── crates/
│   ├── tauri-app/         # Tauri desktop wrapper
│   ├── app/               # Core application logic
│   ├── proxy/             # HTTP proxy implementation
│   ├── tunstack/          # TUN interface handling
│   ├── netlink/           # Linux netlink bindings
│   ├── firewall/          # nftables firewall rules
│   └── ...                # Other utility crates
├── Cargo.toml             # Rust workspace configuration
└── README.md              # This file
```

## Troubleshooting

### "Permission denied" when running

The app needs root privileges. Run with sudo or configure proper capabilities.

### Build fails with missing libraries

Make sure you've installed all system dependencies listed in the Prerequisites section.

### Tauri CLI not found

Run `cargo install tauri-cli` and ensure `~/.cargo/bin` is in your PATH.

### Frontend dev server not connecting

Make sure the Vite dev server is running on port 5173 before starting the Tauri app.

## Documentation

- [Contributing](./CONTRIBUTING.md)
- [Security Policy](./SECURITY.md)

## AI-Assisted Development

This project uses Claude Code for AI-assisted development. See [CLAUDE.md](./CLAUDE.md) for guidelines.

## License

This project is licensed under the MIT License - see the [LICENSE](./LICENSE) file for details.
