//! Opening a map file for editing: load the packs around it and find the map it defines.

use std::path::{Path, PathBuf};

use strata_content::{ContentRegistry, discover_packs, load};
use strata_model::MapDef;
use thiserror::Error;

use crate::document::MapDocument;
use crate::palette::Palette;
use strata_content::RegistryDefs;

/// Why a map file could not be opened.
#[derive(Debug, Error)]
pub enum OpenError {
    /// The file cannot be read.
    #[error("cannot read {path}")]
    Read {
        /// The path as given.
        path: String,
        /// What went wrong.
        #[source]
        source: std::io::Error,
    },
    /// No folder above the file holds a `pack.lua`.
    #[error("{0} is not inside a pack")]
    NotInPack(String),
    /// The packs next to the map's pack could not be listed, or the map's pack is not one.
    #[error("{0}")]
    Discover(String),
    /// The packs do not load; the message is the first error.
    #[error("the packs do not load ({0}); run `strata pack validate`")]
    Load(String),
    /// The file is in a pack but defines no map.
    #[error("{0} defines no map")]
    NoMap(String),
}

/// A map file opened for editing, with everything its packs define.
#[derive(Debug)]
pub struct OpenedMap {
    /// The map's full `pack:name` ID.
    pub id: String,
    /// The file as the registry records it: `pack/relative/path.lua`.
    pub source: String,
    /// The ID of the pack the map belongs to; its names resolve from there.
    pub pack: String,
    /// The canonical path of the file.
    pub file: PathBuf,
    /// Every definition of the map's pack and the packs beside it.
    pub registry: ContentRegistry,
    /// The map as loaded.
    pub map: MapDef,
}

impl OpenedMap {
    /// Opens the map in `file`, loading every pack in the folder that holds the map's pack.
    pub fn open(file: &Path) -> Result<Self, OpenError> {
        let canonical = file.canonicalize().map_err(|source| OpenError::Read {
            path: file.display().to_string(),
            source,
        })?;
        let not_in_pack = || OpenError::NotInPack(canonical.display().to_string());
        let pack_root = canonical
            .ancestors()
            .find(|dir| dir.join("pack.lua").is_file())
            .ok_or_else(not_in_pack)?;
        let roots = [pack_root.parent().unwrap_or(pack_root).to_path_buf()];
        let packs = discover_packs(&roots).map_err(|e| OpenError::Discover(e.to_string()))?;
        let same = |p: &&strata_content::LoadedPack| {
            p.root.canonicalize().ok().as_deref() == Some(pack_root)
        };
        let pack = packs
            .iter()
            .find(same)
            .ok_or_else(|| OpenError::Discover(format!("no pack at {}", pack_root.display())))?;
        let pack = pack.manifest.id.to_string();
        let registry = load(&roots).map_err(|errors| {
            OpenError::Load(errors.first().map(ToString::to_string).unwrap_or_default())
        })?;
        let relative = canonical
            .strip_prefix(pack_root)
            .map_err(|_| not_in_pack())?;
        let source = format!("{pack}/{}", relative.to_string_lossy().replace('\\', "/"));
        let found = registry
            .iter::<MapDef>()
            .find(|(_, _, map)| map.source.file == source)
            .map(|(_, id, map)| (id.to_owned(), map.clone()));
        let (id, map) = found.ok_or_else(|| OpenError::NoMap(source.clone()))?;
        Ok(Self {
            id,
            source,
            pack,
            file: canonical.clone(),
            registry,
            map,
        })
    }

    /// The palette of every tile and feature the packs define, named as this map's pack sees
    /// them.
    pub fn palette(&self) -> Palette {
        Palette::new(&RegistryDefs::new(&self.registry, self.pack.as_str()))
    }

    /// Starts editing the map.
    pub fn into_document(self) -> MapDocument<RegistryDefs> {
        MapDocument::new(self.map, RegistryDefs::new(self.registry, self.pack))
    }
}
