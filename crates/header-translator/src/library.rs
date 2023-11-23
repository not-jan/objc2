use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::io;
use std::path::Path;

use crate::config::LibraryData;
use crate::file::File;

#[derive(Debug, PartialEq, Default)]
pub struct Library {
    pub files: BTreeMap<String, File>,
    link_name: String,
    data: LibraryData,
}

impl Library {
    pub fn new(name: &str, data: &LibraryData) -> Self {
        Self {
            files: BTreeMap::new(),
            link_name: name.to_string(),
            data: data.clone(),
        }
    }

    pub fn output(&self, path: &Path) -> io::Result<()> {
        for (name, file) in &self.files {
            // NOTE: some SDK files have '+' in the file name
            let name = name.replace('+', "_");
            let mut path = path.join(name);
            path.set_extension("rs");
            fs::write(&path, file.to_string())?;
        }

        // truncate if the file exists
        fs::write(path.join("mod.rs"), self.to_string())?;

        Ok(())
    }

    pub fn compare(&self, other: &Self) {
        super::compare_btree(&self.files, &other.files, |name, self_file, other_file| {
            let _span = debug_span!("file", name).entered();
            self_file.compare(other_file);
        });
    }
}

fn prepare_for_docs(s: &str) -> String {
    s.trim().replace('\n', "\n//! ")
}

impl fmt::Display for Library {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = self.data.name.as_deref().unwrap_or(&self.link_name);
        writeln!(
            f,
            "// This file has been automatically generated by `objc2`'s `header-translator`."
        )?;
        writeln!(f, "// DO NOT EDIT")?;
        writeln!(f)?;
        writeln!(f, "//! # Bindings to the `{name}` framework")?;
        if !self.data.extra_docs.is_empty() {
            writeln!(f, "//!")?;
            writeln!(f, "//! {}.", prepare_for_docs(&self.data.extra_docs))?;
        }
        if !self.data.examples.is_empty() {
            writeln!(f, "//!")?;
            writeln!(f, "//!")?;
            let examples_plural = if self.data.examples.len() > 1 {
                "s"
            } else {
                ""
            };
            writeln!(f, "//! ## Example{examples_plural}")?;
            for example in &self.data.examples {
                writeln!(f, "//!")?;
                writeln!(f, "//! {}.", prepare_for_docs(&example.description))?;
                writeln!(f, "//!")?;
                writeln!(f, "//! ```ignore")?;
                writeln!(
                    f,
                    "#![doc = include_str!(\"../../../examples/{}.rs\")]",
                    example.name
                )?;
                writeln!(f, "//! ```")?;
            }
        }
        if self.data.name.is_some() {
            writeln!(f, "#![doc(alias = \"{}\")]", self.link_name)?;
        }
        writeln!(f)?;

        if self.data.has_additions {
            writeln!(f, "#[path = \"../../additions/{name}/mod.rs\"]")?;
            writeln!(f, "mod additions;")?;
            writeln!(f, "pub use self::additions::*;")?;
        }
        writeln!(f)?;
        if self.data.has_fixes {
            writeln!(f, "#[path = \"../../fixes/{name}/mod.rs\"]")?;
            writeln!(f, "mod fixes;")?;
            writeln!(f, "pub use self::fixes::*;")?;
        }
        writeln!(f)?;

        // Link to the correct framework
        //
        // FIXME: We always do cfg_attr(feature = "apple", ...) to make compiling things for GNUStep easier.
        writeln!(
            f,
            "#[cfg_attr(feature = \"apple\", link(name = \"{}\", kind = \"framework\"))]",
            self.link_name
        )?;
        if let Some(gnustep_library) = &self.data.gnustep_library {
            writeln!(
                f,
                "#[cfg_attr(feature = \"gnustep-1-7\", link(name = \"{}\", kind = \"dylib\"))]",
                gnustep_library
            )?;
        }
        writeln!(f, "extern \"C\" {{}}")?;
        writeln!(f)?;

        for name in self.files.keys() {
            // NOTE: some SDK files have '+' in the file name
            let name = name.replace('+', "_");
            writeln!(f, "#[path = \"{name}.rs\"]")?;
            writeln!(f, "mod __{name};")?;
        }

        writeln!(f)?;

        for (name, file) in &self.files {
            // NOTE: some SDK files have '+' in the file name
            let name = name.replace('+', "_");
            for stmt in &file.stmts {
                let mut iter = stmt.declared_types().filter(|item| !item.starts_with('_'));
                if let Some(item) = iter.next() {
                    // Use a set to deduplicate features, and to have them in
                    // a consistent order
                    let mut features = BTreeSet::new();
                    stmt.visit_required_types(|item| {
                        if let Some(feature) = item.feature() {
                            features.insert(format!("feature = \"{feature}\""));
                        }
                    });
                    match features.len() {
                        0 => {}
                        1 => {
                            writeln!(f, "#[cfg({})]", features.first().unwrap())?;
                        }
                        _ => {
                            writeln!(
                                f,
                                "#[cfg(all({}))]",
                                features
                                    .iter()
                                    .map(|s| &**s)
                                    .collect::<Vec<&str>>()
                                    .join(",")
                            )?;
                        }
                    }

                    writeln!(f, "pub use self::__{name}::{{{item}")?;
                    for item in iter {
                        writeln!(f, ", {item}")?;
                    }
                    writeln!(f, "}};")?;
                }
            }
        }

        Ok(())
    }
}
