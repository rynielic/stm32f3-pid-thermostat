.PHONY: test_pid test_crc8 test

CC      = gcc
CFLAGS  = -std=c11 -Wall -Wextra -Wconversion -ICore/Inc -Itests/unity
SRC     = Core/Src/crc8.c Core/Src/pid.c tests/unity/unity.c

test_pid: tests/test_pid.c Core/Src/pid.c Core/Inc/pid.h tests/unity/unity.c
	$(CC) $(CFLAGS) tests/test_pid.c Core/Src/pid.c tests/unity/unity.c -o $@
	./$@

test_crc8:
	$(CC) $(CFLAGS) tests/test_crc8.c Core/Src/crc8.c tests/unity/unity.c -o test_crc8 && ./test_crc8
	
test: test_crc8 test_pid