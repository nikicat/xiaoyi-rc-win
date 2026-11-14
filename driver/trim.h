#pragma once

#include <stddef.h>

// Length of a HID report descriptor without its trailing 0x00 padding.
// Returns len unchanged unless what remains ends in End Collection (0xC0),
// so anything that doesn't look like padded junk is left alone.
size_t TrimReportDescriptor(const unsigned char *desc, size_t len);
