# Ghi chú giao thức iBasso

Lệnh được rút từ app iBasso UAC 2.1.9 (`com.ibasso.volume`) bằng phân tích tĩnh, và cài đặt trong `src/ibasso.rs`.
Mục nào chưa thử trên thiết bị thật đều ghi rõ.

## Chọn bộ điều khiển

App iBasso chọn bộ điều khiển theo chuỗi tên sản phẩm USB (`UacManager`), với VID `0x262A`, `0x2FC6` hoặc `0x0661`.

| Họ | Model | Kênh gửi | Gói lệnh |
| --- | --- | --- | --- |
| HID 64 byte | DC04 Ultra, Macchiato, D18 | interrupt OUT | `4B 01 3F 08 lệnh nhóm` + dữ liệu |
| HID 8 byte | DC-Elite, DC07 Pro, DC-Nunchaku, DC-Tonfa | interrupt OUT | `lệnh nhóm kiểm_tra 00` + 4 byte dữ liệu |
| Thanh ghi | DC06, DC06 Pro, DC03 Pro, DC04 Pro, Macaron | HID SET_REPORT | ghi thanh ghi I2C trong gói 16 byte |

Các cài đặt trong cùng nhóm được ghi chung một gói, nên app chỉ ghi sau khi thiết bị đã báo giá trị hiện tại của nhóm đó.

## DC04 Ultra

Đã đọc được trạng thái thật (firmware 1.54.30): âm lượng, cân bằng, gain, bộ lọc, EQ, DRE, S/PDIF, chế độ ampli, micro, màn hình, ngôn ngữ và 3 bộ EQ tham số.
Đã ghi thử trên máy thật: đổi thời gian sáng màn hình rồi đặt lại, máy báo đúng giá trị mới.

- DC04 Ultra dùng report HID có đánh số: byte đầu của gói 64 byte (`0x4B`) chính là report ID, nên gửi nguyên gói, không chèn thêm byte `00`.
- Máy có thể bỏ sót truy vấn khi nhận quá nhanh. Mỗi truy vấn lúc kết nối chờ phản hồi tới 200 ms và gửi lại một lần, như app iBasso.

## DC03 Pro (và DC04 Pro, Macaron cùng họ)

Thử trên DC03 Pro thật (262a:187e), chỉ đọc:

- Đọc thanh ghi 4 byte của chip DAC (`0x60`, `0x62`) đều trả về `ec ec ec ec`, không phải giá trị thật.
- Thanh ghi bản sao `0xA2` chỉ chứa giá trị app iBasso ghi lần trước, sau khi cắm lại là 0.

App iBasso cũng không đọc trạng thái từ các model này mà ghi lại giá trị đã lưu khi kết nối. Vì vậy app này không đọc gì khi kết nối,
mà gửi lại các giá trị đã lưu lần trước, như app iBasso. Lệnh ghi đã được người dùng xác nhận trên DC03 Pro thật.

## EQ tham số (DC04 Ultra, Macchiato)

Mỗi bộ có 10 dải. Đọc bằng `4B 80 09 1F <bộ+7> <dải>`. Ghi từng dải, rồi lưu (`4B 01 0A 04 <bộ+7> 00 01 01`) và kích hoạt (`4B 01 01 04 00 00 02`).
Trong gói dải: tần số ở byte 28 đến 29, Q ×256 ở byte 30 đến 31, gain ×256 ở byte 32 đến 33 (little-endian), loại bộ lọc ở byte 34, gain tổng ở byte 35.
