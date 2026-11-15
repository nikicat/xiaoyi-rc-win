#pragma once

// Pass/fail reporting shared by the unit tests. A test's main returns
// ExpectExitCode().

#include <stdbool.h>
#include <stdio.h>

static int failures;

// Prints the check's result and counts a failure. Returns ok.
static bool Expect(const char *name, bool ok)
{
    printf("%s %s\n", ok ? "ok  " : "FAIL", name);
    if (!ok) {
        failures++;
    }
    return ok;
}

// Returns 1 if any check failed and 0 otherwise.
static int ExpectExitCode(void)
{
    return failures ? 1 : 0;
}
