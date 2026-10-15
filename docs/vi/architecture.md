<!-- Dịch từ docs/en/architecture.md. Sửa bản gốc rồi dịch lại; đừng sửa bản này. -->
# Kiến trúc

[← Chỉ mục tài liệu](index.md)

camareLOL được xây dựng từ ba thành phần với một quy tắc giữa chúng: **chỉ lõi
Rust được phép thay đổi hệ thống.** Hai thành phần còn lại có thể yêu cầu.

```
┌──────────────────────────────────────────────────────────────────┐
│                        camareLOL                                 │
│                                                                    │
│   ┌────────────┐   ┌──────────────┐   ┌───────────────────────┐  │
│   │  TUI       │   │  Python      │   │  Java catalog         │  │
│   │  (ratatui) │   │  tools       │   │  (package catalog,    │  │
│   │            │   │              │   │   desktop profiles,   │  │
│   │  renders,  │   │  validates,  │   │   plugin interface,   │  │
│   │  navigates │   │  recommends, │   │   JSON-RPC server)    │  │
│   └─────┬──────┘   │  diagnoses   │   └──────────┬────────────┘  │
│         │          └──────┬───────┘              │               │
│         └────────┬────────┴──────────────────────┘               │
│                  │ versioned JSON protocol (NDJSON)              │
│                  ▼                                                │
│   ┌──────────────────────────────────────────────────────────┐  │
│   │              Rust core (authoritative)                    │  │
│   │  hardware · storage · layout · boot · accounts · packages │  │
│   │  config · logging · recovery · install orchestration      │  │
│   │  safety gate — every system-changing request passes here  │  │
│   └──────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────────┘
```

## Trách nhiệm

| Thành phần | Phải làm | Không bao giờ được làm |
| --- | --- | --- |
| **Lõi Rust** | khám phá phần cứng và lưu trữ, lập kế hoạch phân vùng, cấu hình hệ thống tệp và khởi động, điều phối cài đặt, kiểm tra an toàn, thao tác đặc quyền | lưu mật khẩu trên ổ đĩa, chạy lệnh phá huỷ mà không có cổng an toàn đã xác nhận, tin một đường dẫn thiết bị mà không kiểm tra lại |
| **Python** | tự động hoá, kiểm tra cấu hình, đề xuất gói, phân tích phụ thuộc, chẩn đoán, tiện ích kiểm thử | thực hiện thao tác phá huỷ, vượt qua lớp an toàn, tắt âm thầm một tính năng — một runtime bị thiếu phải được **báo cáo** |
| **Java** | danh mục gói có thể mở rộng, hồ sơ desktop, giao diện plugin, mô-đun cấu hình trên một API đã định nghĩa | thực hiện thao tác phá huỷ, vượt qua lớp an toàn |

## Vì sao có một lõi, chứ không phải một script

Thiết kế thay thế là một script shell gọi `sgdisk` và `mkfs`. Nó dễ viết hơn và
không thể suy luận được vào đúng lúc quan trọng nhất: lúc một ổ đĩa sắp bị xoá.

Đặt mọi lệnh thay đổi hệ thống sau một thành phần duy nhất nghĩa là không thể
quên các phép kiểm tra khi gọi. Python và Java dễ thêm tính năng hơn và do đó
dễ có thêm mã mới hơn — nên lõi xác minh mọi yêu cầu từ chúng đúng như nó xác
minh yêu cầu từ TUI.

## Thứ tự build

1. `core/` — thư viện Rust: `cargo build -p camarelol-core`
2. `tui/` — tệp nhị phân `camarelol`, phụ thuộc vào lõi
3. `python/` — không có bước build: `python3 -m unittest discover -s tests`
4. `java/` — `cd java && ./build.sh` (javac, jar, tests)

`./build.sh` ở thư mục gốc kho thực hiện cả bốn.

## Giao thức IPC

Tóm tắt một dòng: JSON phân tách theo dòng mới, với id yêu cầu, phiên bản giao
thức, lỗi có kiểu, thời gian chờ và huỷ theo từng yêu cầu. Xem
[giao thức IPC](ipc-protocol.md).

Chi tiết quan trọng: lõi xác minh **mọi** yêu cầu. Một công cụ Python không thể
đi tới một đường ghi nếu lõi không đồng ý.

## Xử lý sự cố

- Một công cụ ngoài thất bại được báo kèm mã thoát của nó và một đoạn stderr đã
  khử trùng lặp. Bí mật được che trước khi bất cứ thứ gì được ghi.
- Một lần cài đặt dừng ở giai đoạn thất bại **đầu tiên**. Nó không bao giờ đi
  tiếp vào một bước không an toàn, không bao giờ tuyên bố thành công mà chưa
  đạt, và không bao giờ tự chạy lại một bước phá huỷ.
  `recovery::resume_plan` liệt kê các giai đoạn an toàn để thử lại.
- Một ổ đĩa mà danh tính đã đổi kể từ lúc kế hoạch được tạo sẽ huỷ lượt chạy.
- Một runtime bị thiếu được báo cáo: trình cài đặt nói "Python tool not
  available" thay vì giả vờ tính năng đó không tồn tại.

## Mô hình bảo mật

1. **Không nội suy chuỗi vào shell.** Dữ liệu người dùng không bao giờ đi tới
   shell. Mọi công cụ ngoài được gọi với các phần tử argv riêng lẻ, và script
   `sfdisk` được đưa qua stdin thay vì truyền như đối số.
2. **Cổng an toàn.** Một thao tác phá huỷ cần `--allow-destructive` **và** một
   cụm từ xác nhận phải gõ tay khớp với mức rủi ro của nó (`ERASE`,
   `ERASE AND WIPE`).
3. **Danh tính thiết bị.** Một kế hoạch lưu WWN/số sê-ri, không lưu `/dev/sda`.
   Ngay trước mỗi lần ghi, ổ đĩa được quét lại và so sánh với cái đã lập kế
   hoạch.
4. **Không có bí mật lưu trên đĩa.** Mật khẩu chỉ tồn tại trong bộ nhớ. Chúng
   được đưa vào `chpasswd` qua stdin và không bao giờ xuất hiện trong argv, log
   hay tệp cấu hình.
5. **Chạy thử không ghi theo mặc định.** Bất kỳ lệnh ghi nào cũng cần `--execute`.

## Chiến lược kiểm thử

| Lớp | Cách thức |
| --- | --- |
| Đơn vị | `cargo test`, Python `unittest`, Java `CatalogTest` |
| Nghiệm thu | khảo sát thiết bị tổng hợp chạy trên tệp nhị phân đã build, qua `--mock` |
| Toàn hệ thống | một VM VirtualBox với đĩa trống, khởi động từ Arch ISO, chạy trình cài đặt ở chế độ có kịch bản |
| Đóng gói | CI build tệp tarball phát hành và chạy `makepkg` trên nó trong một container Arch, nên một `PKGBUILD` hỏng sẽ làm build thất bại |

Chi tiết ở [testing](testing.md).

## Tiếp theo

- [Giao thức IPC](ipc-protocol.md)
- [Kiểm thử](testing.md)
- [Những gì chưa làm](roadmap.md)
