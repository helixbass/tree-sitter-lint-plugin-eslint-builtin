#![cfg(test)]

use once_cell::sync::Lazy;

use crate::scope::EcmaVersion;

static SUPPORTED_ECMA_VERSIONS: Lazy<Vec<EcmaVersion>> =
    Lazy::new(|| vec![3, 5, 6, 7, 8, 9, 10, 11, 12, 13]);

pub fn get_supported_ecma_versions(min: Option<EcmaVersion>) -> impl Iterator<Item = EcmaVersion> {
    let min = min.unwrap_or_default();

    SUPPORTED_ECMA_VERSIONS
        .iter()
        .copied()
        .filter(move |&ecma_version| ecma_version >= min)
        .flat_map(|ecma_version| {
            if ecma_version >= 6 {
                vec![ecma_version, ecma_version + 2009]
            } else {
                vec![ecma_version]
            }
        })
}
