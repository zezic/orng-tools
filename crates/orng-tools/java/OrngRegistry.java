// SPDX-FileCopyrightText: 2026 Sergey Ukolov
// SPDX-License-Identifier: GPL-3.0-only

// Authoring source for OrngRegistry.j, which is what the crate actually uses.
//
// This is the class preparation injects. Bitwig calls it twice per launch: once
// at the end of the content registry's initialiser, and once at the end of the
// entitlement object's constructor. Both times it reads the entry list and
// applies it, so the archive never has to know what is registered and adding
// content is a file write rather than a patch.
//
// It is compiled against the stubs in placeholder/, whose names are rewritten in
// the constant pool at patch time to whatever this Bitwig build calls them. The
// stubs exist only so this file compiles; they are never shipped and never
// loaded. Everything else here is java.base, which cannot be obfuscated.
//
// Regenerate the assembly after editing:
//
//   javac --release 17 -d <tmp> placeholder/*.java OrngRegistry.java
//   krak2 dis --out OrngRegistry.j <tmp>/OrngRegistry.class
//
// then delete the .linenumbertable and .sourcefile lines.

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.UUID;

import orng.placeholder.Grant;
import orng.placeholder.Kind;
import orng.placeholder.Registry;

public final class OrngRegistry {
    /** Columns of one entry: uuid, kind, name, library path, then the two this class ignores. */
    private static final int UUID_COLUMN = 0;
    private static final int KIND_COLUMN = 1;
    private static final int NAME_COLUMN = 2;
    private static final int PATH_COLUMN = 3;
    private static final int COLUMNS = 4;

    private OrngRegistry() {
    }

    /**
     * Register every entry with the content registry.
     *
     * Called from the registry's own class initialiser, after its own entries,
     * so ours are added to a table that is already complete.
     */
    public static void install() {
        try {
            for (String[] row : entries()) {
                Kind kind = Enum.valueOf(Kind.class, row[KIND_COLUMN]);
                Registry.register(
                        UUID.fromString(row[UUID_COLUMN]),
                        row[NAME_COLUMN],
                        kind,
                        row[PATH_COLUMN],
                        false);
            }
        } catch (Throwable failure) {
            report(failure);
        }
    }

    /**
     * Grant every registered identity in each of the three maps.
     *
     * The maps are passed in rather than read off the entitlement object,
     * so that this class needs to know none of its field names.
     *
     * Which map holds which kind is deliberately not worked out: a UUID
     * identifies exactly one document, so granting it as all three kinds is
     * the same in effect as granting it as the right one.
     */
    public static void grant(HashMap<UUID, Object> devices,
                             HashMap<UUID, Object> modulators,
                             HashMap<UUID, Object> modules) {
        try {
            for (String[] row : entries()) {
                UUID uuid = UUID.fromString(row[UUID_COLUMN]);
                devices.put(uuid, new Grant(uuid));
                modulators.put(uuid, new Grant(uuid));
                modules.put(uuid, new Grant(uuid));
            }
        } catch (Throwable failure) {
            report(failure);
        }
    }

    /**
     * The entry list, tab separated, or nothing at all if there is none.
     *
     * A missing file is the normal state of an installation with nothing
     * registered. A short line is skipped rather than refused: the writer
     * validates every field before it gets here, so a line that is wrong now is
     * a file someone edited by hand, and dropping one entry beats dropping all
     * of them.
     */
    private static List<String[]> entries() throws Exception {
        Path path = Path.of(System.getProperty("user.home"), ".orng", "entries.tsv");
        List<String[]> rows = new ArrayList<>();
        if (!Files.isReadable(path)) {
            return rows;
        }
        for (String line : Files.readAllLines(path)) {
            if (line.isEmpty() || line.charAt(0) == '#') {
                continue;
            }
            String[] columns = line.split("\t");
            if (columns.length >= COLUMNS) {
                rows.add(columns);
            }
        }
        return rows;
    }

    /**
     * Never throw into Bitwig.
     *
     * This runs inside a class initialiser and a constructor that Bitwig cannot
     * start without. An exception escaping either would stop the application
     * rather than lose a device, so the only correct failure here is a message
     * and an installation that behaves as though nothing were registered.
     */
    private static void report(Throwable failure) {
        System.err.println("orng-registry: could not apply the entry list: " + failure);
    }
}
