//! Where the repo is, a uQ config tree for one generated file, and `which`.

use std::path::{Path, PathBuf};
use uq_core::paths::Paths;
use uq_core::stub::stub_contents;

pub fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Lay out a uQ config tree for one theme variant; returns (paths, main config file).
pub fn tree(root: &Path, generated: &str) -> (Paths, PathBuf) {
    let p = Paths::under(root);
    std::fs::create_dir_all(p.base_lua.parent().unwrap()).unwrap();
    std::fs::copy(repo().join("config/hypr/base.lua"), &p.base_lua).unwrap();
    std::fs::create_dir_all(p.generated_lua.parent().unwrap()).unwrap();
    std::fs::write(&p.generated_lua, generated).unwrap();
    std::fs::write(&p.user_lua, "").unwrap();
    std::fs::write(&p.main_lua, stub_contents(&p)).unwrap();
    let main = p.main_lua.clone();
    (p, main)
}

pub fn which(bin: &str) -> Result<PathBuf, ()> {
    std::env::var_os("PATH")
        .and_then(|paths| {
            std::env::split_paths(&paths)
                .map(|d| d.join(bin))
                .find(|p| p.is_file())
        })
        .ok_or(())
}
