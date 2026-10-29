# Embedded licenses

Settings → Show licenses works offline. It includes File Beholder's MIT license, Material Design icon and font notices, and the locked Windows dependency inventory.

`dependencies.json` is generated from Cargo's active normal/build dependency graph. It records its Windows target and Cargo.lock SHA-256. Package source paths and local usernames are not written to the inventory. Each dependency includes its license expression, upstream URL, and the retained license/notice text.

The `overrides/` directory supplies texts omitted from a published crate's archive. Its manifest records exact upstream revisions, URLs, and SHA-256 hashes. The `fonts/` directory retains Fira Sans's license and source checksum. The `sk-iced/` directory retains the pinned shared UI library's published licenses and notice unchanged. Historical product names within those upstream texts refer to that library's distribution.

After changing dependencies, populate Cargo's cache with a normal build, then regenerate and verify:

```powershell
python scripts/generate-licenses.py --target x86_64-pc-windows-gnu
python scripts/generate-licenses.py --check
```

Use the release's actual Windows target. `--check` defaults to the target recorded in the retained inventory and fails if its contents differ from the locked graph. Regeneration uses only cached Cargo metadata and package license files; it does not fetch license texts from the network.
