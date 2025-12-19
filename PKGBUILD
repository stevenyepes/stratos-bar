# Maintainer: Steven Yepes
pkgname=stratos-bar
pkgver=0.1.0
pkgrel=1
pkgdesc="A sophisticated command palette for power users"
arch=('x86_64')
url="https://github.com/yourusername/stratos-bar"
license=('MIT')
depends=('webkit2gtk-4.1' 'gtk3' 'libappindicator-gtk3' 'openssl')
makedepends=('npm' 'cargo' 'nodejs' 'git')
source=("${pkgname}::git+file://${startdir}")
sha256sums=('SKIP')

pkgver() {
    cd "$srcdir/$pkgname"
    # Generate version based on package.json or git tag
    # For now, just use the hardcoded version or a git revision
    # printf "r%s.%s" "$(git rev-list --count HEAD)" "$(git rev-parse --short HEAD)"
    echo "0.1.0"
}

prepare() {
    cd "$srcdir/$pkgname"
    # install dependencies if needed here, but usually done in build
}

build() {
    cd "$srcdir/$pkgname"
    # Install frontend dependencies
    npm install
    
    # We use tauri build. Note: This will try to bundle, but we are interested in the binary and resources.
    # We restrict to 'deb' bundle to avoid AppImage requirements (linuxdeploy) which fail in this environment.
    npm run tauri -- build --bundles deb
}

package() {
    cd "$srcdir/$pkgname"
    
    # Install binary
    install -Dm755 "src-tauri/target/release/stratos-bar" "$pkgdir/usr/bin/stratos-bar"
    
    # Install icons
    # Assuming icons are available in src-tauri/icons
    install -Dm644 "src-tauri/icons/32x32.png" "$pkgdir/usr/share/icons/hicolor/32x32/apps/stratos-bar.png"
    install -Dm644 "src-tauri/icons/128x128.png" "$pkgdir/usr/share/icons/hicolor/128x128/apps/stratos-bar.png"
    install -Dm644 "src-tauri/icons/128x128@2x.png" "$pkgdir/usr/share/icons/hicolor/256x256/apps/stratos-bar.png"
    install -Dm644 "src-tauri/icons/icon.png" "$pkgdir/usr/share/icons/hicolor/512x512/apps/stratos-bar.png"
    
    # Install desktop entry if available, otherwise create one
    # Tauri generates one in target/release/bundle/deb/... but we can just create a simple one here or use a provided one.
    
    mkdir -p "$pkgdir/usr/share/applications"
    cat > "$pkgdir/usr/share/applications/stratos-bar.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=StratosBar
Comment=A sophisticated command palette for power users
Exec=stratos-bar
Icon=stratos-bar
Categories=Utility;
EOF
}
