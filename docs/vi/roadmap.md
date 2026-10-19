<!-- Dịch từ docs/en/roadmap.md. Sửa bản gốc rồi dịch lại; đừng sửa bản này. -->
# Lộ trình

[← Chỉ mục tài liệu](index.md)

Những gì đã làm, những gì chưa, và — ở chỗ chưa — vì sao.

## Stage 1 — MVP

Hoàn tất.

- [x] Khung TUI với điều hướng bằng bàn phím và phần trợ giúp
- [x] Kiểm tra môi trường: firmware, CPU, RAM, mạng
- [x] Phát hiện lưu trữ qua `lsblk` / `findmnt` / `blkid`, kèm đánh giá rủi ro
- [x] Bố trí tự động cho ổ trống: GPT và MBR, ESP, BIOS boot, root,
      `/home`, swap
- [x] Chọn hệ thống tệp: ext4, btrfs, xfs
- [x] Locale, keymap, múi giờ
- [x] Hostname, tài khoản root, tài khoản người dùng, chính sách sudo chống khoá
      tài khoản
- [x] Cài Arch tối giản: pacstrap, fstab, trình nạp khởi động, service
- [x] Ghi log và các màn hình xác nhận
- [x] Cổng an toàn: khoá, chạy thử không ghi, thực thi với cụm từ phải gõ tay
- [x] Lưu và nạp cấu hình, có phiên bản schema, không có mật khẩu
- [x] ~~Trình tự cập nhật có ký, có dự phòng mirror~~ — đã bỏ, và đây là trạng
      thái đúng. Cập nhật đến từ `pacman` hoặc tải tay. Một thành phần việc của
      nó là thay thế một binary xoá ổ đĩa là con đường ngắn nhất từ máy chủ bị
      chiếm tới dữ liệu bị xoá, và chuỗi mirror đứng sau nó hỏng liên tục trong
      khi vẫn báo thành công.
- [x] Lớp tự động hoá Python, danh mục Java, IPC có phiên bản

## Một mục chỉ được tick khi có bằng chứng

Mọi `[x]` ở trên đều đã đối chiếu với một lần cài thật trong VirtualBox, không
phải với việc "code có nhắc tới". Bốn tên gói đã qua sạch mọi unit test rồi
chỉ chết trong máy ảo — `greetd-tuigreeter` (Hyprland và Sway không cài được),
`p7zip` (đã bị gỡ khỏi kho Arch), và `systemd-boot` (không hề là một gói; mọi
bản cài chọn systemd-boot đều chết ở `pacstrap`). Một ô tick chỉ có nghĩa là
"code có nhắc tới" thì vô dụng với một trình cài đặt.

Hai lần thất bại trong quá trình là **bài kiểm tra sai**, không phải installer
sai: khẳng định display manager phải tạo symlink trong `multi-user.target.wants/`
(nó dùng `Alias=display-manager.service` và không tạo gì cả), và `test -e` trên
symlink tuyệt đối đó — nó đi theo root của máy chủ thay vì đi vào trong mount.

`[~]` là một phần: đã có và đã kiểm chứng phần đó, phần còn lại chưa tồn tại.


## Stage 2 — Desktop

- [x] Hồ sơ desktop: KDE, GNOME, Xfce, Cinnamon, LXQt, Hyprland, Sway — đã
      chứng minh bằng một lần cài thật trong VirtualBox (`pacstrap` 26 gói,
      `hyprland` có thật trên đĩa)
- [x] Wayland hoặc X11, được chọn kèm kiểm tra tương thích — Sway bị từ chối
      với X11; lựa chọn cụ thể kéo đúng một stack
- [x] Trình quản lý phiên hiển thị: SDDM, GDM, LightDM, greetd — được cài,
      được bật, và greetd tự viết `/etc/greetd/config.toml`
- [~] Phát hiện driver và gợi ý firmware — **một phần**: có gợi ý microcode theo
      CPU và một bộ gói nền (`mesa`, `linux-firmware`). Chưa dò được model GPU.
- [x] PipeWire, PulseAudio, hoặc không dùng cái nào
- [x] Lựa chọn Bluetooth, zram, swap
- [~] Cấu hình NetworkManager — **một phần**: gói đã cài, unit đã bật. Chưa
      viết tệp `/etc/NetworkManager`.

## Stage 3 — Lưu trữ nâng cao

- [ ] Dual-boot: dùng lại một ESP sẵn có, giữ nguyên các phân vùng Windows,
      recovery và MSR
- [ ] Trình soạn thảo phân vùng thủ công: kích thước, thứ tự, hệ thống tệp, điểm
      gắn
- [ ] Mã hoá LUKS
- [ ] Thu nhỏ NTFS, với các phép kiểm tra an toàn đã viết sẵn trong đặc tả
- [ ] Bố trí subvolume btrfs

## Stage 4 — Trải nghiệm

- [x] Lưu và tiếp tục một cấu hình qua nhiều lượt chạy — nạp lại profile đã
      lưu vào wizard bằng phím `L` ở màn hình chào; file hỏng hoặc sai
      `schema_version` bị từ chối thay vì lặng lẽ thay bằng mặc định, và mật
      khẩu không bao giờ được khôi phục (`#[serde(skip)]`).
- [x] Tìm kiếm và chọn gói — `/` lọc danh mục, phím mũi tên di chuyển, Space
      bật/tắt. Gói AUR chỉ được đưa ra khi bật AUR; tên lấy từ danh mục,
      không tự bịa.
- [x] Hỗ trợ AUR, build bằng người dùng thường và không bao giờ bằng root —
      `makepkg` thủ công cộng nhận diện `yay`/`paru`/`pikaur`/`trizen`. Ba lớp
      chặn root độc lập; không tự cài helper nào.
- [x] Xuất báo cáo chẩn đoán — `--save-diagnostics <FILE>` ghi báo cáo đã che
      bí mật (host, khảo sát đĩa, kế hoạch, các pha, cảnh báo, đường dẫn log).
- [x] Thêm ngôn ngữ cho giao diện — tiếng Việt, tiếng Anh, tiếng Trung giản thể.
- [x] Hướng dẫn khôi phục tốt hơn — khi cài đứt giữa chừng, TUI liệt kê pha đã
      xong, pha hỏng và phần an toàn để thử lại, và nói rõ không hứa máy sẽ
      khởi động được.

## Cố ý hoãn lại

Hai mục được nêu ra vì việc thiếu chúng trông như một sự sơ suất, nhưng không
phải vậy.

**Thu nhỏ NTFS.** Tiêu chí nghiệm thu đã có và các phép kiểm tra an toàn đã
được viết ra, nhưng tính năng thì chưa làm. Thu nhỏ một phân vùng đang giữ bản
sao Windows duy nhất của ai đó là thao tác một chiều, không hoàn tác được, và nó
cần nhiều kiểm thử hơn mức dự án này hiện có thể đưa ra.

**Dùng lại một ESP sẵn có cho dual-boot.** Quy tắc phát hiện và giữ nguyên đã
được hiện thực, và mã lập kế hoạch biết cách để yên các phân vùng Windows.
Phần còn thiếu là giao diện bố trí phân vùng, và phát hành một nửa sẽ có nghĩa
là đưa cho ai đó một kế hoạch mà họ không thể xác nhận một cách an toàn.

Trong cả hai trường hợp, việc thành thật là nói ra thay vì đưa ra một ô tick mà
làm ít hơn vẻ ngoài.

## Tiếp theo

- [Kiến trúc](architecture.md)
- [Kiểm thử](testing.md)
