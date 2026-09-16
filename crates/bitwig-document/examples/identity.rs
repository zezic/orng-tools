// SPDX-FileCopyrightText: 2026 Sergey Ukolov
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Print the identity of every document given on the command line.

fn main() {
    for arg in std::env::args().skip(1) {
        let path = std::path::Path::new(&arg);
        match bitwig_document::Document::read(path) {
            Ok(doc) => {
                let id = doc.identity();
                println!(
                    "{:<22} {:<10} uuidv{} {}  {:?}",
                    id.name,
                    format!("{:?}", doc.kind()),
                    id.uuid.get_version_num(),
                    id.uuid,
                    doc.serialization()
                );
            }
            Err(e) => println!("{arg}: {e}"),
        }
    }
}
