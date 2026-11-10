#include "unity.h"
#include "crc8.h"

void setUp(void) {}
void tearDown(void) {}

void test_empty_input_returns_zero(void)
{
    const uint8_t dummy[1] = {0};
    TEST_ASSERT_EQUAL_HEX8(0x00, crc8(dummy, 0));
}

void test_standard_check_value(void)
{
    const uint8_t msg[] = "123456789";   /* 9 characters; the terminator is not counted */
    TEST_ASSERT_EQUAL_HEX8(0xA1, crc8(msg, 9));
}

void pass_one_byte_less(void)
{
    const uint8_t msg[] = "123456789";   /* 9 characters; the terminator is not counted */
    TEST_ASSERT_EQUAL_HEX8(0xA1, crc8(msg, 8));
}

void flip_every_bit_in_valid_message(void)
{
	uint8_t buf[9] = {0x50, 0x05, 0x4B, 0x46, 0x7F, 0xFF, 0x0C, 0x10, 0x00};
	buf[8] = crc8(buf, 8);

	TEST_ASSERT_EQUAL_HEX8(0x00, crc8(buf, 9));

	int undetected = 0;

	for (size_t i = 0; i < 9; i++) {
		for (unsigned bit = 0; bit<8; bit++) {
			buf[i] = (uint8_t)(buf[i] ^ (1U << bit));

            if (crc8(buf, 9) == 0x00) {
                undetected++;
            }

            buf[i] = (uint8_t)(buf[i] ^ (1U << bit));
		}
	}
    TEST_ASSERT_EQUAL_INT(0, undetected);
}

int main(void)
{
    UNITY_BEGIN();
    RUN_TEST(test_empty_input_returns_zero);
    RUN_TEST(test_standard_check_value);
    RUN_TEST(pass_one_byte_less);
    RUN_TEST(flip_every_bit_in_valid_message);
    return UNITY_END();
}
