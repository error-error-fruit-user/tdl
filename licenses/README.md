# Licenses for crates that ship none

A few crates linked into tdl are published without a license file, so `publish.py` can't copy one
into `THIRD_PARTY_LICENSES.txt`. Their license texts are kept here instead, each taken from the
crate's own repository, and `publish.py` uses them.

For a crate called `name`, it uses `name.txt`, or else the file for the part of the name before
the first `-`. So `symphonia.txt` covers every `symphonia-*` crate and `wezterm.txt` covers
`wezterm-input-types`.

When `publish.py` reports a crate with no license anywhere, find the license in its repository
and add it here the same way.
