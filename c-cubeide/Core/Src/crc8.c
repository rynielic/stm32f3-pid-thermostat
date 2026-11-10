#include <stdint.h>
#include <stddef.h>

uint8_t crc8(const uint8_t *data, size_t len)
{
	uint8_t crc = 0;
	for (size_t i = 0; i < len; i++) {
		uint8_t byte = data[i];
		for (int bit = 0; bit < 8; bit++) {
			uint8_t mix = (crc ^ byte) & 0x01;
			crc >>= 1;
			if (mix != 0) {
				crc ^= 0x8C;
			}
			byte >>= 1;
		}
	}
	return crc;
}

