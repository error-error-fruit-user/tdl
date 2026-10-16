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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tag_is_the_first_free_number_counting_on_from_any_already_there() {
        let taken = ["brutal", "brutal_2", "brutal_4"];
        let free = |name: &str| !taken.contains(&name);
        assert_eq!(tagged("brutal", free), "brutal_3");
        assert_eq!(tagged("brutal_2", free), "brutal_3");
        assert_eq!(tagged("brutal_4", free), "brutal_5");
        assert_eq!(tagged("sunlust", free), "sunlust_2");
        // only a number after the last `_` counts as one
        assert_eq!(tagged("doom_ii", free), "doom_ii_2");
        assert_eq!(tagged("_7", free), "_7_2");
    }

    #[test]
    fn a_files_tag_goes_in_front_of_its_extension() {
        let taken = ["brutal_2.pk3"];
        let free = |name: &str| !taken.contains(&name);
        assert_eq!(tagged_file("brutal.pk3", free), "brutal_3.pk3");
        assert_eq!(tagged_file("brutal_2.pk3", free), "brutal_3.pk3");
        assert_eq!(tagged_file("DOOM2.WAD", free), "DOOM2_2.WAD");
        assert_eq!(tagged_file("notes", free), "notes_2");
        assert_eq!(tagged_file(".wad", free), ".wad_2");
        assert_eq!(split_extension("a.b.pk3"), ("a.b", ".pk3"));
    }
}
