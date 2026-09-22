// SPDX-License-Identifier: GPL-3.0-or-later
#include "apps/host/linux/src/plank_topology.h"

#include <cassert>
#include <cstdint>
#include <iostream>

int main() {
    namespace topology = plank::topology;
    static_assert(topology::feature_file_clipboard == 0x800000U);
    static_assert((topology::feature_flags & topology::feature_file_clipboard) == 0U,
                  "secure default must not advertise file clipboard");
#if defined(__linux__) && defined(SUNSHINE_BUILD_X11)
    static_assert(topology::feature_platform_file_clipboard ==
                  topology::feature_file_clipboard);
#else
    static_assert(topology::feature_platform_file_clipboard == 0U);
#endif

    const auto enabled = topology::feature_flags |
                         topology::feature_platform_file_clipboard;
    assert((enabled & topology::feature_clipboard_sync) != 0U);
#if defined(__linux__) && defined(SUNSHINE_BUILD_X11)
    assert((enabled & topology::feature_file_clipboard) != 0U);
#endif
    std::cout << "File clipboard negotiation: allocation and secure default passed\n";
}
