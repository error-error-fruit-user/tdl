//! Telling a copy apart from its original by a number on the end: `brutal_2`, then `brutal_3`.
//! Used for pasted profiles and for pasted wads whose file name is already in the profile.

/// `name` with the first number on the end that `is_free` accepts, starting from 2. A name that
/// already ends in `_<number>` counts up from it rather than adding another, so a copy of
/// `brutal_2` is `brutal_3`, not `brutal_2_2`.
pub fn tagged(name: &str, is_free: impl Fn(&str) -> bool) -> String {
    let (base, from) = match name.rsplit_once('_') {
        Some((base, number)) if !base.is_empty() => match number.parse::<u32>() {
            Ok(number) => (base, number.saturating_add(1)),
            Err(_) => (name, 2),
        },
        _ => (name, 2),
    };
    (from..)
        .map(|number| format!("{base}_{number}"))
        .find(|tagged| is_free(tagged))
        .expect("there are always more numbers than names in use")
}

/// A file name split into its stem and its extension (with the dot), so a tag goes in front of
/// the extension: `brutal.pk3` is `("brutal", ".pk3")`. A name with no extension, or only one
/// (`.wad`), is all stem.
pub fn split_extension(file_name: &str) -> (&str, &str) {
    match file_name.rfind('.') {
        Some(dot) if dot > 0 => file_name.split_at(dot),
        _ => (file_name, ""),
    }
}

/// `file_name` with a tag in front of its extension: `brutal_2.pk3`.
pub fn tagged_file(file_name: &str, is_free: impl Fn(&str) -> bool) -> String {
    let (stem, extension) = split_extension(file_name);
    let stem = tagged(stem, |stem| is_free(&format!("{stem}{extension}")));
    format!("{stem}{extension}")
}
