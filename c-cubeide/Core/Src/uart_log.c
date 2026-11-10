#include <stdarg.h>
#include <stdio.h>
#include "main.h"

extern UART_HandleTypeDef huart1;

void uart_printf(const char *fmt, ...)
{
	  char msg[64];

	  va_list args;
	  va_start(args,fmt);
	  int len = vsnprintf(msg, sizeof(msg), fmt, args);
	  va_end(args);

	  if (len < 0) {return;}
	  if ((size_t)len >= sizeof(msg)) {
		  len = (int)sizeof(msg) - 1;
	  }
	  HAL_UART_Transmit(&huart1, (uint8_t *)msg, (uint16_t)len, 100);
}


