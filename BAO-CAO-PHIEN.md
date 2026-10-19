# Báo cáo phiên làm việc — camareLOL

**Ngày:** 19/10/2026 · **Nhánh:** `main` · **Commit cuối:** `0ca6f45`

Câu hối trung thực nhất cho báo cáo này: *những gì đã **chứng minh** bằng
máy ảo thật, và những gì mới chỉ **được unit test bảo vệ**.*

---

## 1. Đã hoàn thành và kiểm chứng

### npm — đã publish thật
```
@wenker-ai/camarelo  0.1.16
@wenker-ai/camarelo  0.1.17   ← dist-tags.latest
```
Xác nhận qua registry, không phải qua thông báo của lệnh.

### Bốn lỗi tên gói Arch — sửa và kiểm chứng
| Viết sai | Đúng | Hậu quả nếu không sửa |
|---|---|---|
| `greetd-tuigreeter` | `greetd-tuigreet` | Hyprland và Sway **không cài được** |
| `p7zip` | `7zip` | profile có archiving → `pacstrap` fail |
| `systemd-boot` (như gói) | không tồn tại | **mọi** bản cài systemd-boot đều chết |

Cả ba đều qua sạch mọi unit test và chỉ chết trong VirtualBox. Đó là lý do
dự án có bài kiểm tra VM bắt buộc trước khi phát hành.

### VM acceptance — 8/8 (nhiều lần)
```
PASS download / scan / selftest / help / install_profile
PASS install    12/12 phase, Outcome: completed
PASS zram       7 mốc thật trên đĩa đích
PASS desktop    en_greetd=enabled · en_cups=enabled · greetd_conf=yes
```
Bằng chứng cụ thể trên đĩa sau khi cài Hyprland + cups:
```
display-manager.service -> /usr/lib/systemd/system/greetd.service
/etc/greetd/config.toml  → command = "tuigreet --time --cmd Hyprland"
```

### Hai lỗi bài kiểm tra (không phải installer)
1. Khẳng định display manager tạo symlink trong `multi-user.target.wants/` —
   sai, nó dùng `Alias=display-manager.service` và không tạo gì ở đó
2. `test -e` trên symlink tuyệt đối → đi theo root máy chủ thay vì đi vào
   mount

Cả hai đều báo **installer hỏng oan**. Đã sửa test, không sửa installer.

### `--preview` — kiểm chứng bằng VM, không chỉ unit test
```
RC=0
DISK_UNCHANGED          ← lsblk trước và sau giống hệt
ls /mnt → No such file  ← không mount gì
```
Chạy với **cả `--allow-destructive` và `--execute`** — vẫn không ghi byte nào.
Đây là điểm chặn duy nhất ở tầng `sys`, không có API nào mở lại được.

Lần test đầu ra `RC=1` và nguyên nhân **là đĩa test 8 GiB của tôi quá
nhỏ**, không phải lỗi tính năng. Đã kiểm trước khi kết luận.

---

## 2. Đã viết xong nhưng CHƯA kiểm chứng bằng VM

### Nhận diện OS + init system
Đọc `/etc/os-release`, **không khởi động tiến trình nào**.

| Phát hiện | Kết luận |
|---|---|
| `ID=arch` + systemd | CHO PHÉP |
| `ID=arch` + OpenRC/runit/dinit | TỪ CHỐI |
| Artix, Nyarch | TỪ CHỐI |
| **Parabola, Hyperbola** | TỪ CHỐI — **không phải Arch**, libre |
| ID khác kể cả `ID_LIKE=arch` | TỪ CHỐI |

**Quyết định quan trọng:** `ID` là chuẩn; `ID_LIKE` **không bao giờ** dùng để
nâng một `ID` lạ thành Arch. Artix, Manjaro, EndeavourOS đều in
`ID_LIKE=arch` — kiểm tra nguồn sẽ lọt hết.

> **Máy này là EndeavourOS (`ID=endeavouros`), nên camareLOL nay từ chối
> chính máy bạn.** Chạy `--self-test` sẽ in:
> `host detected: EndeavourOS / systemd — REFUSED`.
> Đây là đúng thiết kế, không phải hỏng.

### Tiếng Trung
`UiLanguage::Chinese` theo đúng cơ chế sẵn có (vi/en), dùng lại từ vựng
trong `docs/zh` để khớp tài liệu.

### AUR — makepkg thủ công + helper
Không bao giờ build bằng root, ba lớp chặn độc lập: bắt buộc có
`aur_build_user` · bọc `runuser -u<user> --` · `ensure_not_root()` ngay trước
mỗi spawn. Helper: `yay`, `paru`, `pikaur`, `trizen`; không có thì nói rõ,
không tự cài. Tên gói bắt đầu bằng `-` bị chặn — `makepkg` sẽ đọc nó là
tuỳ chọn và "thành công" mà không cài gì.

---

## 3. CHƯA làm — và tôi không báo là đã xong

### Stage 3 — toàn bộ (5 mục)
- Dual-boot dùng lại ESP có sẵn
- Trình soạn thảo phân vùng thủ công
- **Mã hoá LUKS**
- Thu nhỏ NTFS
- Bố trí subvolume btrfs

**Lý do:** đây là những thao tác **phá huỷ dữ liệu không hoàn tác được**.
Trong phiên này đã có bốn lỗi chỉ lộ ra ở máy ảo. Viết LUKS và thu nhỏ NTFS
mà không có người ngồi cạnh xem kết quả là tự tạo ra đúng loại rủi ro mà
dự án cảnh báo. Tôi chọn **không làm** thay vì làm cho có.

### Stage 4 — 3 mục sau
- Tìm kiếm gói
- Nạp cấu hình trong TUI
- Hướng dẫn khôi phục tốt hơn

### Ba việc còn treo khác
1. **Bản vá thread nền** (`5c5082a`) — đã commit, **chưa test được trong VM**
   vì phím Enter không tới được khi tôi tiêm phím. Đây là nguyên nhân khả
   dĩ nhất của "chả ghi gì cũng chả cài gì" mà bạn gặp.
2. **Chưa cắt release 0.1.18** — chứa `--preview`, thread nền, nhận diện
   host, tiếng Trung, AUR.
3. **Lịch sử git không `bisect` được** — hai commit của tôi chạy `git add -A`
   giữa lúc subagent đang sửa dở, lôi code dở vào commit (`337c6d1`). Lỗi
   của tôi.

---

## 4. Việc bảo trì

- **Đĩa:** 96% đầy → **61%** (trống 3.0G → 26G)
- Xoá 2 VM test (11.5G) + cache: JetBrains, yay, cargo `target/`, gradle,
  ms-playwright, uv, go-build · FreesmLauncher · nostalgia
- Xoá **672 file log**
- **Giữ nguyên:** VS Code, opencode CLI, npm/node, JDK, Firefox, Zalo,
  VirtualBox, kitty, Telegram, Sober/Roblox, **serpantinum**, toàn bộ cấu
  hình và dữ liệu cá nhân, cùng VM `camarelol-test` chứa bản Arch của bạn

---

## 5. Khi bạn quay lại — theo thứ tự này

1. **Chạy `vm-test.sh` trên Arch ISO.** Mọi thứ mới thêm chỉ có unit test
   bảo vệ. Đây là bước bắt buộc, không phải tuỳ chọn.
2. **Test tay bản vá thread nền**: mở TUI, gõ `erase`, xem màn Progress có
   chạy từng pha không.
3. **Thử AUR bằng `makepkg` thật một lần** trên Arch — chưa rõ `runuser` có
   thoả kiểm tra root riêng của từng helper không.
4. Nếu 1–3 xanh thì cắt **release 0.1.18**.
5. Gỡ `freesmlauncher-bin`: `sudo pacman -Rns freesmlauncher-bin`.

## 6. Lời nhắc

camareLOL **từ chối chính máy bạn đang dùng** (EndeavourOS). Đó là chủ ý —
`ID_LIKE=arch` không đủ để tin là Arch. Muốn chạy thì dùng **Arch live ISO**,
đúng như tài liệu nói từ đầu.