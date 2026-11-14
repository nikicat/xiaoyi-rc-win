#include "trim.h"

size_t TrimReportDescriptor(const unsigned char *desc, size_t len)
{
    size_t end = len;

    while (end > 0 && desc[end - 1] == 0x00) {
        end--;
    }
    if (end == 0 || desc[end - 1] != 0xC0) {
        return len;
    }
    return end;
}
