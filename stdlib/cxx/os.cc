// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

#include <stdint.h>
#include <string.h>
extern "C" {
#include <hyper/syscall.h>
#include <hyper/thread.h>
#include <hyper/stream.h>
#include <hyper/std.h>
}

extern "C" [[noreturn]] void hyper_cxx_fatal(const char *message)
{
	// Runtime stream selectors are 0/1/2, not startup purpose IDs.
	size_t actual = 0;
	const char *prefix = "C++ runtime failure: ";
	hyper_runtime_stdio_write(2, prefix, strlen(prefix), &actual);
	hyper_runtime_stdio_write(2, message, strlen(message), &actual);
	hyper_runtime_stdio_write(2, "\n", 1, &actual);
	hyper_runtime_stream_fence(2);
	hyper_process_exit(1);
}

extern "C" uint64_t hyper_cxx_clock(uint32_t realtime)
{
	auto result = realtime ? hyper_clock_get_realtime() : hyper_clock_get_monotonic();
	if (result.status)
		hyper_cxx_fatal("clock read failed");
	if (!realtime)
		return result.value0;
	if (result.value1 >= 1000000000 ||
	    result.value0 > (UINT64_MAX - result.value1) / 1000000000)
		hyper_cxx_fatal("realtime outside representable range");
	return result.value0 * 1000000000 + result.value1;
}

extern "C" void hyper_cxx_sleep(uint64_t ns)
{
	auto now = hyper_cxx_clock(0);
	auto deadline = ns >= UINT64_MAX - now ? UINT64_MAX - 1 : now + ns;
	if (hyper_thread_sleep(deadline))
		hyper_cxx_fatal("thread sleep failed");
}

extern "C" void hyper_cxx_yield()
{
	if (hyper_thread_yield())
		hyper_cxx_fatal("thread yield failed");
}
