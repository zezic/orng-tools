// SPDX-FileCopyrightText: 2026 Sergey Ukolov
// SPDX-License-Identifier: GPL-3.0-only

package orange.placeholder;

import java.util.UUID;

/** Stands in for Bitwig's content registry, and for its one registration method. */
public final class Registry {
    private Registry() {
    }

    public static void register(UUID uuid, String name, Kind kind, String libraryPath, boolean flag) {
        throw new AssertionError("placeholder");
    }
}
