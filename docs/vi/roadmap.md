<!-- Dịch từ docs/en/roadmap.md. Sửa bản gốc rồi dịch lại; đừng sửa bản này. -->
# Lộ trình

[← Chỉ mục tài liệu](index.md)

Những gì đã làm, những gì chưa, và — ở chỗ chưa — vì sao.

**Bản hiện tại:** `1.0.3`, beta thứ tư (28/10/2026). Bản này sửa việc disk
virtio bị gán nhãn `SATA HDD` trong khảo sát lưu trữ — chỉ là hiển thị, kèm test
hồi quy; phần lập kế hoạch không đọc trường đó. `1.0.2` khiến `--self-test` báo
`SKIP`, exit 2, trên máy không hỗ trợ thay vì FAIL; `1.0.1` lọc
zram/ram/nbd/md/loop khỏi bước dò đĩa. `1.0.0` là beta đầu tiên: mã cài đặt
0.1.22 cùng đợt rà soát roadmap và cập nhật tài liệu. Thẻ npm `latest` vẫn là
0.1.22 cho tới khi một bản beta được thăng cấp.

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
- [x] ~~Trình tự cập nhật có ký, có dự phòg mirror~~ — đã bỏ, và đây là trạng
      thái đúng. Cập nhật đến từ `pacman` hoặc tải tay. Một thành phần việc của
      nó là thay thế một binary xoá ổ đĩa là con đường ngắn nhất từ máy chủ bị
      chiếm tới dữ liệu bị xoá, và chuỗi mirror đứng sau nó hỏng liên tục trong
      khi vẫn báo thành công.
- [x] Lớp tự động hoá Python, danh mục Java, IPC có phiên bản

## Một mục chỉ được tick khi có bằng chứngn

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

0.1.18 tìm thêm được hai lỗi theo đúng cách đó: nhãn ESP `CAMARELOL-EFI` dài
hơn giới hạn 11 ký tự của FAT nên `mkfs.fat` từ chối mọi bản cài UEFI ở pha
format; và pha mount đi theo thứ tự khai hoạch, root mount đè lên ESP nên
`grub-install` chê `/boot`. Cả hai qua sạch mọi unit test, cả hai chỉ chết
trong VM, và cả hai nay có test canh riêng — giới hạn nhãn theo từng loại
filesystem, và thứ tự mount root-trước-ESP.

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
      CPU và một bộ gói nền (`mesa`, `linux-firmware`), và gợi ý driver nay
      được chọn theo model GPU đã tách từ `lspci -mm -nn` — `nvidia-open` trên
      Turing trở lên, `nvidia` trên Maxwell/Pascal/Volta (model không đọc
      được không bao giờ bị đoán là Turing), thêm `vulkan-radeon` +
      `libva-mesa-driver` cho AMD và `vulkan-intel` + `intel-media-driver`
      cho Intel; GPU ảo thì không thêm gì. Đã unit-test với dòng `lspci`
      thật, kèm test catalog rằng mọi tên nó có thể sinh ra đều tìm được.
      Chưa được quan sát trong VM có GPU thật — đó là lý do mục này vẫn
      "một phần" chứ chưa tick.
- [x] PipeWire, PulseAudio, hoặc không dùng cái nào
- [x] Lựa chọn Bluetooth, zram, swap
- [~] Cấu hình NetworkManager — **một phần**: gói đã cài, unit đã bật, và
      `/etc/NetworkManager/conf.d/00-camarelol.conf` ghim `dhcp=internal`
      (đã kiểm bằng unit-test; chưa được nhìn thấy trong VM).
- [~] Entry firmware cũ — **một phần**: pha bootloader nay đọc `efibootmgr -v`
      và chỉ xóa entry `Boot####` mà `/File(…)` của nó không còn trên ESP CỦA
      TA (đối sánh PARTUUID); `FvFile`/`CdRom`/`Usb` và ESP của OS khác không
      bao giờ bị đụng tới, và listing hỏng cho phép skip kèm ghi chú thay vì
      làm gãy install. Hiểu đúng kiểu treo `Boot0005` — firmware chỉ vào tệp
      `EFI/…` mà lần thử trước để lại. Đã kiểm bằng unit-test với listing
      dáng VBox; chờ quan sát trong VM.

## Stage 3 — Lưu trữ nâng cao (chưa phát hành)

Đã làm trong working tree sau `1.0.3`; chưa nằm trong bản phát hành nào.

- [~] Mã hoá phân vùng gốc bằng LUKS2 — **một phần**: pha format/mount đặt
      container LUKS2 lên phân vùng gốc, format và mount qua mapper
      (`/dev/mapper/cryptroot`), viết `/etc/crypttab`, thêm hook `encrypt` và
      `keyboard` vào `HOOKS` trước `filesystems`, ghi
      `cryptdevice=UUID=…:cryptroot root=/dev/mapper/cryptroot` vào
      `GRUB_CMDLINE_LINUX`, cài `cryptsetup` trong hệ đích, và kiểm mật khẩu
      ở preflight (tối thiểu 8 ký tự, đếm theo ký tự chứ không theo byte).
      Mật khẩu không bao giờ vào hồ sơ hay vào dòng lệnh: nó nằm trong bộ nhớ,
      tới `cryptsetup` qua stdin, và bản xem trước chỉ ghi một chỗ giữ chỗ.
      Những bố cục v1 không thể mã hoá cho trọn vẹn bị từ chối thay vì làm nửa
      vời — `/home` tách riêng, systemd-boot, và dual-boot cạnh OS khác. Đã
      unit-test đầu-cuối trên runner mô phỏng; **chưa VM nào boot một bản cài
      đã mã hoá**, nên lời nhắc của initramfs, sự phối hợp crypttab/hook và
      cmdline của GRUB vẫn chưa được chứng minh ngoài unit test.
      `--luks-passphrase-file` cấp mật khẩu cho bản cài không giao diện.
- [~] Bố cục subvolume btrfs — **một phần**: gốc btrfs được format một lần,
      mount ở tầng đỉnh (`subvolid=5`), tạo `@`, `@home` (chỉ khi `/home`
      không phải phân vùng riêng) và `@snapshots`, rồi mount lại gốc từ `@`
      với `/home` và `/.snapshots` từ subvolume của chúng. Thứ tự và lần
      `umount` giữa hai lần mount gốc đã có unit-test; đầu ra `genfstab` cho
      các mount đó chưa được quan sát trong VM, và chưa cài công cụ snapshot
      nào — đây là bố cục mà công cụ snapshot cần, không phải công cụ đó.
- [ ] Trình soạn thảo phân vùng thủ công — chưa có mã.

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

## Stage 5 — Lên kệ phát hành

- [x] Chuỗi phát hành: `scripts/make-release.sh` build từ thẻ đúng bản, từ chối
      cây làm việc bẩn, bắt binary mới dựng phải tự báo đúng thẻ mà nó được
      build từ (lỗi kiểu 0.1.10 từng rơi vào chính cái bẫy này), và chặn phát
      hành nếu VM acceptance không đạt — chỉ `SKIP_VM=1` bỏ qua cổng, và việc
      bỏ qua là một quyết định tường minh, có ghi lại.
- [x] Harness VM acceptance tự động — khách gửi kết quả của từng bước về host
      qua NAT, không cần Guest Additions, nên một bước treo không nuốt được
      kết quả của các bước trước nó. `camarelol-acceptance` chạy 7/7 với
      binary 0.1.18.
- [x] GitHub Release với mọi tài sản có kiểm băm — binary, tarball mã nguồn,
      jar catalog Java, PKGBUILD sinh ra ghim sha256 của chính tarball, và tệp
      `SHA256SUMS` phủ hết. Đã phát hành 0.1.18.
- [x] Gói npm `@wenker-ai/camarelo` — mỗi lần pack tải binary từ GitHub Release
      đúng phiên bản trong `package.json`, đối chiếu `SHA256SUMS`, bắt
      `--version` phải khớp, chạy test gói rồi mới pack; publish là một bước
      riêng, tường minh. 1.0.0 là beta đầu tiên và 1.0.3 là bản hiện tại, tất
      cả đều dưới dist-tag `beta`; `latest` vẫn là 0.1.22 cho tới khi thăng
      cấp.

## Stage 6 — Dual-boot cạnh một hệ điều hành có sẵn

- [x] Planner giữ nguyên mọi thứ (`plan_dual_boot`) — dựng từ survey thực:
      đĩa GPT+UEFI có phân vùng đóng góp toàn bộ chúng thành những dòng
      `format=false`, ESP của nó được dùng lại tại `/boot/efi`, và bố cục mới
      được đặt bằng start sector tường minh bên trong khe trống thẳng hàng
      lớn nhất — planner từ chối lập kế hoạch khi yêu cầu không lọt nổi. Survey
      phải mang cột START của phân vùng thì thứ này mới tồn tại nổi.
- [x] Đường ghi append-only — `sgdisk --new=5:<start>:<end>` chỉ thêm các
      phân vùng mới; không zap, không viết lại bảng, và các pha format/mount
      gọi đúng số phân vùng thật của từng phân vùng được giữ.
- [x] GRUB trên ESP dùng lại cùng os-prober — `os-prober` vào danh sách
      pacstrap và `/etc/default/grub` nhận `GRUB_DISABLE_OS_PROBER=false`,
      cả hai chỉ khi kế hoạch là dual-boot.
- [x] Bằng chứng VM — cài cạnh một Windows giả và boot được —
      `scripts/vm-dualboot.sh` dựng một đĩa mang dáng Windows (ESP, MSR,
      C: 6 GiB, khôi phục 1 GiB — mỗi cái một tệp đánh dấu), cài cạnh đó và
      kiểm tra mọi tệp đánh dấu còn nguyên, root rơi vào partition 5 mới,
      GRUB nằm trên ESP gốc, `os-prober` được cài, và sau cài đúng 5 phân
      vùng (ba run xanh trọn vẹn, 22 Oct 2026). Run thứ ba một mình chứng
      minh cổng từ chối: đặt firmware BIOS, planner từ chối với câu
      "dual-boot chỉ lập kế hoạch cho đĩa UEFI + GPT". Một lần boot từ đĩa
      chứng minh chuỗi khởi động: NVRAM → `EFI/camareLOL/` trên ESP dùng
      lại → nhân Arch → initramfs → màn hình đăng nhập.
- [x] Entry Windows trong menu boot — Windows giả nay mang đúng cặp mà
      probe `efi/20microsoft` của os-prober đòi hỏi (`bootmgfw.efi` cùng
      một `BCD`), và `grub.cfg` sinh ra đã có
      `menuentry "Windows Boot Manager (on /dev/sda1)"` chainload vào đó —
      harness kiểm bằng `windows_entry ≥ 1` (run 7, 17 kiểm xanh,
      22 Oct 2026). Hai bản sửa làm điều này thành thật chứ không may mắn:
      cờ `GRUB_DISABLE_OS_PROBER=false` nay do chính pha bootloader viết
      (trước đây nằm ở system-config — pha chạy SAU `grub-mkconfig` — cờ
      tới muộn, kể cả Windows thật cũng sẽ bị bỏ lỡ), và yêu cầu BCD được
      tìm ra nhờ traced `os-prober` ngay trong VM. Điều CHƯA nhìn thấy:
      entry menu thực sự boot vào một Windows thật — vẫn hoãn bên dưới.
      Nhắc lại lần nữa: **chưa được chỉ camareLOL vào đĩa đang giữ bản
      Windows duy nhất của bạn.** Các bước khởi đầu cho máy thật nằm
      trong `docs/DUAL-BOOT-TEST.md`.
- [x] Máy thật — cài cạnh EndeavourOS và boot được — một lần chạy thật của
      `scripts/dual-boot-auto.sh` (0.1.22) thêm partition `camarelol-root`
      vào khe trống của đĩa thật 119 GiB: ESP 1 GiB có sẵn và root
      EndeavourOS 69 GiB giữ nguyên `format=false`, root ext4 ~49 GiB mới
      nằm trong khe để lại sau khi xoá partition đầu tiên. Lần boot đầu
      đã xác nhận trên chính máy đó (24 Oct 2026): F11 chọn `camareLOL`
      trong menu firmware, GRUB nạp từ `EFI/CAMARELOL/` trên ESP dùng lại,
      hệ mới tới màn hình đăng nhập desktop. Chưa nhìn thấy trong nhánh
      này: đường quay về — boot EndeavourOS tiếp qua entry
      `endeavouros (EFI BootNext)` đã sinh, và bước đổi thứ tự mặc định
      `efibootmgr -o 0002,0000` — cả hai chờ lần reboot tới; đợt dọn rác
      entry cũ (`EFI/archtest` + `Boot0001`, `menuentry` ISO boot trong
      `grub.cfg` của hệ cũ, file ISO 1.6 GiB trong `/mnt/Storage`) cũng
      chưa xong.

## Ý tưởng tương lai (chỉ là ý tưởng, chưa code)

**siroLOL — thứ thay Ventoy/Rufus mà không cần USB.** Phác thảo bởi tác
giả ngày 24-10-2026, ngày thứ 13 của quá trình phát triển: thay vì USB,
trải một partition FAT32 trên đĩa, giải nén ISO vào một thư mục ở đó
(giới hạn file 4 GiB của FAT32 khiến việc bỏ nguyên file vào là không
được), và boot như Ventoy/Rufus đã làm, với menu boot mức partition.
Bộ ngôn ngữ định hướng: Rust, C, C++, shell, HTML/CSS, Makefile. Một
điểm cộng song hành: partition FAT32 boot được trên nhiều firmware hơn
nhiều so với loopback ext4, chính vì mọi UEFI đều đọc được FAT32. Chưa
có gì tồn tại — không thiết kế, không repo, không một dòng code — và nó
không đổi gì trong chính camareLOL. Hai hình thái được hình dung: một
GUI chạy trên Windows (dễ dùng hơn, chỉ hỗ trợ FAT32), và hình thái được
hỗ trợ tốt nhất — TUI/CLI trên Linux, danh sách chọn lên xuống, tự tải
.iso về, nhận diện logo, và một phần cố ý làm thủ công.

**tOrsiLOL — bản phân phối theo preset-script.** Cũng chỉ là phác thảo:
cài cho người dùng Linux các mẫu script `.nix`, `.lua`, `.sh` được lựa
kỹ (tự cài và thiết lập Hyprland, tăng bảo mật, làm đẹp, v.v), điều
khiển bằng GUI. Lưu ý thành thật ghi kèm: script `.nix` chỉ có nghĩa
đầy đủ trên NixOS hoặc với Nix package manager — hoặc Nix đi kèm các
preset, hoặc cách đặt tên sẽ gây hiểu lầm rằng `.nix` nào cũng chạy
được ở đâu cũng được. Xếp sau siroLOL: khi đã có đường cài chuẩn,
các preset của tOrsiLOL mới có chỗ mà bám vào. Cái này cũng chưa có
gì cả — không thiết kế, không repo, không code.

## Cố ý hoãn lại

Những mục sau được nêu ra vì việc thiếu chúng trông như một sự sơ suất, nhưng
không phải vậy.

**Thu nhỏ NTFS.** Tiêu chí nghiệm thu đã có và các phép kiểm tra an toàn đã
được viết ra, nhưng tính năng thì chưa làm. Thu nhỏ một phân vùng đang giữ bản
sao Windows duy nhất của ai đó là thao tác một chiều, không hoàn tác được, và nó
cần nhiều kiểm thử hơn mức dự án này hiện có thể đưa ra.

**Dual-boot cạnh một Windows thật.** Phần giữ-nguyên-mọi-thứ và entry menu
Windows đã done và tick trong Stage 6 ở trên — được VM chứng minh với Windows
giả. Còn hoãn là thứ chỉ một Windows thật mới trả lời được: nhìn thấy entry
menu thực sự boot vào Windows (bootmgr giả được chainload nhưng không làm gì
cả), và toàn bộ quy trình thu nhỏ từ bên trong Windows. Không một bản phát
hành nào của camareLOL nên lại gần đĩa đang giữ bản Windows duy nhất của
người dùng cho tới khi chặng này khép lại.

**Phần còn lại của lưu trữ nâng cao.** Mã hoá gốc bằng LUKS2 và bố cục
subvolume btrfs nay đã có code, có test, và những chặng chưa chứng minh được
ghi rõ ở Stage 3 phía trên; phần vẫn hoãn là mã hoá `/home` riêng, mã hoá
cạnh hệ điều hành khác, và gốc đã mã hoá qua systemd-boot — planner từ chối
cả ba thay vì làm gần đúng. **Trình soạn thảo phân vùng thủ công** vẫn chưa có
một dòng code nào, và danh sách stage từng mang nó như một ô tick nói ít hơn
vẻ ngoài của nó.

Trong mọi trường hợp ở đây, việc thành thật là nói ra thay vì đưa ra một ô
tick mà làm ít hơn vẻ ngoài.

## Tiếp theo

- [Kiến trúc](architecture.md)
- [Kiểm thử](testing.md)
