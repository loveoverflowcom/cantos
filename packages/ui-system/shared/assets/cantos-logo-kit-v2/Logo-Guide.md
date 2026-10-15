# Cantos · Logo Kit v2

Giữ biểu tượng **C + sóng âm** từ design Cantos đã có. Chuẩn hóa khoảng cách biểu tượng / chữ, thêm lockup Studio và các màu dùng trên nền sáng, tối hoặc in đơn sắc.

| File | Dùng khi |
| --- | --- |
| cantos-logo.svg / .png | Logo ngang chính trên nền sáng |
| cantos-studio-logo.svg / .png | Cantos Studio trên web / tài liệu |
| cantos-symbol.svg / .png | Avatar, app icon, favicon, dấu nhận diện nhỏ |
| Các file có -dark | Nền tối; mực lavender sáng và biểu tượng đổi tương phản |
| Các file có -mono | In một màu hoặc dùng watermark; khoảng âm thật sự trong suốt |
| cantos-wordmark.svg / .png | Chỉ cần chữ Cantos, không có biểu tượng |

SVG là vector, mọi chữ đã chuyển thành path, không có font ngoài hoặc ảnh bitmap nhúng. PNG dùng nền trong suốt (RGBA): biểu tượng 1024 × 1024; các lockup cao 512 px. SVG phù hợp web, in ấn và resize; PNG phù hợp chèn trực tiếp vào tài liệu hoặc ứng dụng không hỗ trợ SVG.

Màu chính: `#675080`. Bản nền tối: `#D9BDF1`. Bản đơn sắc: `#292331`. Dùng nguyên tỷ lệ; khoảng trống tối thiểu quanh phần vẽ bằng 1/4 chiều cao biểu tượng. Gợi ý kích thước nhỏ nhất: biểu tượng 24 px, logo ngang rộng 140 px, logo Studio rộng 160 px. Các số này là hướng dẫn đề xuất, nên kiểm tra đọc được ở kích thước sử dụng thực tế.

Không kéo giãn, đổi riêng màu một nét trong biểu tượng hoặc thêm viền quanh chữ. Khi logo nhỏ, dùng biểu tượng riêng. PNG đã trong suốt; nền giấy / nền tối trong `Cantos-Logo-Kit-Preview.png` chỉ để xem cách phối nền.

Dựng lại SVG: Python 3 + fontTools, `python3 source/export-logo.py`. Dựng PNG: Node + sharp, `node source/export-png.mjs`. Fonts Noto Serif Regular / Noto Sans Bold và giấy phép OFL được kèm trong `source/fonts` để chỉnh sửa chữ sau này.
