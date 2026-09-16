// SPDX-FileCopyrightText: 2026 Sergey Ukolov
// SPDX-License-Identifier: GPL-3.0-only

package orange.placeholder;

/**
 * Stands in for Bitwig's content category enum.
 *
 * The constant names are the real ones. An enum keeps them readable through
 * obfuscation because Enum.valueOf needs them, which is what makes looking a
 * category up by name a safe anchor.
 */
public enum Kind {
    DEVICE,
    MODULATOR,
    MODULE
}
