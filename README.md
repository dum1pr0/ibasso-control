# iBasso Control

App desktop viết bằng Rust để điều khiển DAC/amp USB của iBasso trên Windows và Linux.

> Đây là dự án cá nhân, không chính thức, không liên quan và không được iBasso hỗ trợ. "iBasso" và tên các model là thương hiệu của chủ sở hữu tương ứng. Dùng với rủi ro của bạn.

## Tải về

Bản build sẵn cho Windows và Linux có ở trang [Releases](../../releases). Bản Windows là một file `.exe` chạy ngay, không cần cài đặt. Bản Linux là file `.tar.gz` gồm file chạy và icon.

## Model hỗ trợ

DC03 Pro, DC04 Pro, DC04 Ultra, DC06, DC06 Pro, DC07 Pro, DC-Elite, DC-Tonfa, DC-Nunchaku, Macaron, Macchiato, D18.

Lệnh được rút từ app iBasso UAC 2.1.9 (`com.ibasso.volume`). Đã thử trên máy thật: đọc và ghi trên DC04 Ultra, ghi trên DC03 Pro. Các model khác chưa được thử trên thiết bị thật.

## Biên dịch từ mã nguồn

### Windows

1. Cài [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) với mục **Desktop development with C++** (cần MSVC và Windows SDK).
2. Cài Rust bằng [rustup](https://rustup.rs) (hoặc `winget install Rustlang.Rustup`), chọn toolchain mặc định `stable-x86_64-pc-windows-msvc`.
3. Trong PowerShell, tại thư mục dự án:

```powershell
cargo build --release
```

File chạy nằm ở `target\release\ibasso-control.exe` (một file duy nhất, có thể chép sang máy khác). Chạy luôn bằng:

```powershell
.\target\release\ibasso-control.exe
```

Windows không cần driver hay quyền admin: DAC dùng driver HID có sẵn.

### Linux

1. Cài Rust:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

2. Cài thư viện hệ thống (chọn lệnh theo bản phân phối):

```bash
# Debian, Ubuntu, Linux Mint, Pop!_OS
sudo apt install build-essential pkg-config libudev-dev libxkbcommon-dev libgl1-mesa-dev libwayland-dev libx11-dev

# Fedora
sudo dnf install gcc pkgconf-pkg-config systemd-devel libxkbcommon-devel mesa-libGL-devel wayland-devel libX11-devel

# Arch, Manjaro
sudo pacman -S --needed base-devel pkgconf systemd-libs libxkbcommon mesa wayland libx11
```

3. Biên dịch và chạy:

```bash
cargo build --release
./target/release/ibasso-control
```

Lần đầu, bấm **Cài quyền udev** cạnh thiết bị (xem mục dưới), rồi rút và cắm lại DAC.

Để app hiện trong menu ứng dụng với icon riêng (tùy chọn):

```bash
install -Dm755 target/release/ibasso-control ~/.local/bin/ibasso-control
install -Dm644 assets/icon.png ~/.local/share/icons/hicolor/256x256/apps/ibasso-control.png
cat > ~/.local/share/applications/ibasso-control.desktop <<'EOF'
[Desktop Entry]
Type=Application
Name=iBasso Control
Exec=ibasso-control
Icon=ibasso-control
Categories=AudioVideo;Audio;
EOF
```

### Icon

Icon được vẽ bằng code trong `src/icon.rs`. Trên Windows, `build.rs` nhúng `assets/icon.ico` vào file exe. Sau khi sửa thiết kế, tạo lại file icon bằng:

```powershell
cargo run --example make_icon
```

## Quyền USB trên Linux

Mỗi thiết bị phát hiện được có nút **Cài quyền udev**. Nút này tạo một file riêng `/etc/udev/rules.d/70-ibasso-control-VID-PID.rules`, hỏi quyền qua `pkexec` (hoặc `sudo -A` nếu máy không có `pkexec`), rồi nạp lại udev. Sau đó hãy rút và cắm lại thiết bị.

## Chạy ngầm và khởi động cùng máy

- Khi mở, app đặt một icon ở khay hệ thống (system tray). Nút **X** chỉ ẩn cửa sổ, app vẫn chạy và giữ kết nối với DAC.
- Mở lại cửa sổ bằng cách bấm vào icon (hoặc chọn **Mở iBasso Control** trong menu), hoặc mở lại app như bình thường. App chỉ chạy một bản: lần mở thứ hai sẽ gọi cửa sổ của bản đang chạy lên.
- Thoát hẳn bằng **Thoát** trong menu của icon khay, hoặc **Thoát** trong menu chuột phải của nút app trên taskbar Windows (hoặc kết thúc tiến trình trong Task Manager).
- Công tắc **Chạy khi khởi động máy** nằm trong trang **Cài đặt** (nút ở góc dưới bên trái), cùng với chế độ màu. Khi bật, app tự mở khi đăng nhập, ẩn dưới khay:
  - Windows: thêm mục `iBasso Control` vào `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` (thấy ở tab Startup của Task Manager).
  - Linux: tạo file `~/.config/autostart/ibasso-control.desktop` theo chuẩn XDG.
  - Cả hai chỉ áp dụng cho tài khoản hiện tại, không cần quyền admin hay sudo. Tắt công tắc sẽ xóa mục đó.
- Trên Linux, icon dùng chuẩn StatusNotifierItem: KDE, XFCE, Cinnamon có sẵn. GNOME cần extension "AppIndicator and KStatusNotifierItem Support". Nếu không thấy icon, mở lại app để gọi cửa sổ lên. Trên Wayland, app chạy qua XWayland để có thể ẩn và hiện lại cửa sổ.

## Ghi chú khi dùng

- Với máy đọc được trạng thái (DC04 Ultra, DC06, DC07 Pro, DC-Elite...), kết nối chỉ đọc, không đổi cài đặt. Trong lúc đọc app hiện thanh tiến độ và khóa các nút chỉnh.
- DC03 Pro, DC04 Pro và Macaron không cho đọc cài đặt hiện tại. App ghi nhớ lần chỉnh gần nhất của từng máy (trong `%APPDATA%\ibasso-control\settings.txt`, hoặc `~/.config/ibasso-control/settings.txt` trên Linux) và tự gửi lại ngay khi kết nối, giống app iBasso gốc. Âm lượng được gửi trước.
- Với DC04 Ultra, Macchiato, D18, DC07 Pro, DC-Nunchaku và DC-Tonfa, thanh cân bằng dùng thang 0 đến 20, trong đó 10 là giữa.
- EQ tham số (DC04 Ultra, Macchiato) chỉ ghi được sau khi đã đọc đủ cả 3 bộ từ thiết bị.
- App không có chức năng cập nhật firmware.

## Công cụ chẩn đoán

Các ví dụ sau chỉ đọc, không đổi cài đặt:

```powershell
cargo run --example hid_probe
cargo run --example hid_status
cargo run --example hid_snapshot
```

## Phát hành bản mới

GitHub Actions (`.github/workflows/build.yml`) chạy test và build cho Windows và Linux ở mỗi lần push. Khi đẩy một tag bắt đầu bằng `v`, nó tạo Release kèm file chạy:

```powershell
git tag v0.3.0
git push origin v0.3.0
```

## Giấy phép

MIT, xem file [LICENSE](LICENSE).
