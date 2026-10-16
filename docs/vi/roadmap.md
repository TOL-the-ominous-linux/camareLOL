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

## Stage 2 — Desktop

- [ ] Hồ sơ desktop: KDE, GNOME, Xfce, Cinnamon, LXQt, Hyprland, Sway
- [ ] Wayland hoặc X11, được chọn kèm kiểm tra tương thích
- [ ] Trình quản lý phiên hiển thị: SDDM, GDM, LightDM, greetd
- [ ] Phát hiện driver và gợi ý firmware
- [ ] PipeWire, PulseAudio, hoặc không dùng cái nào
- [ ] Lựa chọn Bluetooth, zram, swap
- [ ] Cấu hình NetworkManager

## Stage 3 — Lưu trữ nâng cao

- [ ] Dual-boot: dùng lại một ESP sẵn có, giữ nguyên các phân vùng Windows,
      recovery và MSR
- [ ] Trình soạn thảo phân vùng thủ công: kích thước, thứ tự, hệ thống tệp, điểm
      gắn
- [ ] Mã hoá LUKS
- [ ] Thu nhỏ NTFS, với các phép kiểm tra an toàn đã viết sẵn trong đặc tả
- [ ] Bố trí subvolume btrfs

## Stage 4 — Trải nghiệm

- [ ] Lưu và tiếp tục một cấu hình qua nhiều lượt chạy
- [ ] Tìm kiếm và chọn gói
- [ ] Hỗ trợ AUR, build bằng người dùng thường và không bao giờ bằng root
- [ ] Xuất báo cáo chẩn đoán
- [ ] Thêm ngôn ngữ cho giao diện
- [ ] Hướng dẫn khôi phục tốt hơn

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
