# Maintainer: Your Name <your.email@example.com>
pkgname=proxyvpn
pkgver=0.1.0
pkgrel=1
pkgdesc="Route TCP traffic through HTTP CONNECT proxy via TUN device"
arch=('x86_64' 'aarch64')
url="https://github.com/yourusername/proxyvpn"
license=('MIT')
depends=('gcc-libs' 'nftables')
makedepends=('cargo' 'git')
optdepends=(
    'iptables: alternative to nftables for packet marking'
)
backup=()
install=proxyvpn.install
source=("$pkgname-$pkgver.tar.gz::$url/archive/v$pkgver.tar.gz")
# For local development, use:
# source=("git+file:///home/kiel/work/http-tun")
sha256sums=('SKIP')

# For git source:
# pkgver() {
#     cd "$pkgname"
#     git describe --long --tags 2>/dev/null | sed 's/^v//;s/\([^-]*-g\)/r\1/;s/-/./g' || echo "0.1.0.r$(git rev-list --count HEAD).g$(git rev-parse --short HEAD)"
# }

prepare() {
    cd "$srcdir/$pkgname-$pkgver"
    export RUSTUP_TOOLCHAIN=stable
    cargo fetch --locked --target "$(rustc -vV | sed -n 's/host: //p')"
}

build() {
    cd "$srcdir/$pkgname-$pkgver"
    export RUSTUP_TOOLCHAIN=stable
    export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-target}"
    cargo build --frozen --release -p proxyvpn
}

check() {
    cd "$srcdir/$pkgname-$pkgver"
    export RUSTUP_TOOLCHAIN=stable
    export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-target}"
    cargo test --frozen --release -p proxyvpn-app --lib
}

package() {
    cd "$srcdir/$pkgname-$pkgver"
    local _target_dir="${CARGO_TARGET_DIR:-target}"

    # Install binary
    install -Dm755 "${_target_dir}/release/proxyvpn" "$pkgdir/usr/bin/proxyvpn"

    # Install documentation
    install -Dm644 "README.md" "$pkgdir/usr/share/doc/$pkgname/README.md"

    # Install license (create one if it doesn't exist)
    if [[ -f "LICENSE" ]]; then
        install -Dm644 "LICENSE" "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
    fi
}
