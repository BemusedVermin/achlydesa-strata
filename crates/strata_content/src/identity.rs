//! Content hashes and pack-set identity (design §4.3 step 6, §4.5).

use std::fmt;
use std::path::Path;

use semver::Version;
use sha2::{Digest, Sha256};
use strata_kernel::PackId;
use strata_lua::FrozenRecord;

use crate::error::PackError;
use crate::manifest::LoadedPack;

/// A SHA-256 digest of loaded content.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentHash(pub [u8; 32]);

impl fmt::Display for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.iter().try_for_each(|byte| write!(f, "{byte:02x}"))
    }
}

impl fmt::Debug for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ContentHash({self})")
    }
}

/// Which pack, which version, and a hash of its definitions and scripts.
///
/// Replays record one of these per pack and verify them before the first command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackIdentity {
    /// The pack's ID.
    pub id: PackId,
    /// The pack's version.
    pub version: Version,
    /// Covers the pack's frozen definitions and the bytes of its `.lua` files.
    pub content_hash: ContentHash,
}

/// Hashes each pack in `order`. `records` must be in `(kind, ID)` order, as `freeze_resolved`
/// returns them; a record belongs to the pack named by the start of its ID.
///
/// The hash covers each record's canonical `postcard` bytes, then each `.lua` file under the
/// pack root, ordered by relative path, as path and bytes. A top-level `tests` directory is
/// skipped, since pack tests cannot change behavior.
pub(crate) fn pack_identities(
    packs: &[LoadedPack],
    order: &[PackId],
    records: &[FrozenRecord],
) -> Result<Vec<PackIdentity>, PackError> {
    let mut identities = Vec::with_capacity(order.len());
    for id in order {
        let pack = packs.iter().find(|p| p.manifest.id == *id);
        let pack = pack.expect("load order names a discovered pack");
        let prefix = format!("{id}:");
        let mut hasher = Sha256::new();
        for record in records.iter().filter(|r| r.record.id.starts_with(&prefix)) {
            let bytes =
                postcard::to_allocvec(&(&record.record.kind, &record.record.id, &record.def))
                    .expect("a frozen definition serializes");
            feed(&mut hasher, &bytes);
        }
        let mut scripts = Vec::new();
        collect_scripts(&pack.root, &pack.root, &mut scripts)?;
        scripts.sort();
        for (path, file) in &scripts {
            feed(&mut hasher, path.as_bytes());
            let bytes = std::fs::read(file).map_err(|source| PackError::Io {
                path: file.clone(),
                source,
            })?;
            feed(&mut hasher, &bytes);
        }
        identities.push(PackIdentity {
            id: id.clone(),
            version: pack.manifest.version.clone(),
            content_hash: ContentHash(hasher.finalize().into()),
        });
    }
    Ok(identities)
}

/// A hash of every pack's identity in load order, so a different set, version or content gives a
/// different hash.
pub(crate) fn set_hash(identities: &[PackIdentity]) -> ContentHash {
    let mut hasher = Sha256::new();
    for identity in identities {
        let version = identity.version.to_string();
        feed(&mut hasher, identity.id.as_str().as_bytes());
        feed(&mut hasher, version.as_bytes());
        feed(&mut hasher, &identity.content_hash.0);
    }
    ContentHash(hasher.finalize().into())
}

/// Length-prefixes `bytes`, so adjacent fields cannot run together.
fn feed(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

/// Collects `(relative path with '/' separators, absolute path)` of every `.lua` file.
fn collect_scripts(
    root: &Path,
    dir: &Path,
    out: &mut Vec<(String, std::path::PathBuf)>,
) -> Result<(), PackError> {
    let io = |source| PackError::Io {
        path: dir.to_owned(),
        source,
    };
    for entry in std::fs::read_dir(dir).map_err(io)? {
        let path = entry.map_err(io)?.path();
        let relative = path.strip_prefix(root).unwrap_or(&path);
        if path.is_dir() {
            if relative != Path::new("tests") {
                collect_scripts(root, &path, out)?;
            }
        } else if path.extension().is_some_and(|ext| ext == "lua") {
            let parts: Vec<_> = relative.iter().map(|p| p.to_string_lossy()).collect();
            out.push((parts.join("/"), path));
        }
    }
    Ok(())
}
