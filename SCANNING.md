# Size scanning

Directory entries and file lengths are read on background workers. File sizes are logical lengths, not allocated/compressed disk space. Symlinks and reparse subtrees are skipped to avoid loops; inaccessible/skipped entries make totals partial. Hard links count per path. Named alternate streams are not included.

## Modes

- **Auto** (default): use a shared read-only NTFS MFT index where available, then recursive traversal for unsupported/inaccessible volumes or missing directory records.
- **MFT**: only indexed totals; unavailable sizes remain unknown, with no recursive fallback.
- **Recursive**: only filesystem traversal; never opens a raw NTFS volume.
- **Off**: keep ordinary file metadata, without traversing directories for their sizes.

Scanning does not elevate itself. Administrator access may enable raw NTFS reads; the separate launch/elevation settings control that choice. Non-NTFS and network paths use recursion in Auto.

## NTFS index

The backend queries FSCTL_GET_NTFS_VOLUME_DATA and the MFT extent map, then reads mapped data in 4 MiB batches. If metadata-stream access is denied but raw reads are available, it can bootstrap the DATA run map from record zero. Unavailable extension extents cause fallback rather than assuming a contiguous MFT.

The parser validates sector fixups and attribute bounds, handles resident/nonresident unnamed DATA, skips DOS aliases, joins extension records using full references and aggregates the parent graph. Directory handles provide file IDs for cached total lookup. Unsupported metadata, torn reads, cycles and inconsistent extent mappings do not produce guessed totals. The MFT record limit is 16 million to bound memory.

Concurrent tabs share index construction. Successful snapshots expire after 30 seconds; failed attempts are cached for five seconds. Refresh forces rebuilding. There is no persistent index, VSS transaction or USN-journal replay, so a live volume may change after the snapshot.

## Progressive updates and cancellation

Recursive traversal samples partial totals every 128 entries and on completion. Results are coalesced into at most one UI batch per 125 ms, plus a final flush. Partial markers remain until a total is finalized. Indexed totals become available after the MFT snapshot has been validated and use the same batched delivery.

Navigation cancels prior work; generation and tab identity checks reject stale results. Cancellation is checked during traversal, reads, aggregation and cache waits; an OS call already in progress may complete first. Closed result channels cancel the worker.

The UI adjusts totals incrementally and preserves scroll identity. Re-sorting is limited to size/share sorts or size filters. Only the bounded virtualized row window is rebuilt. Advanced search caches recursive directory totals per subtree rather than repeatedly walking descendants.

Binary/Metric/Match OS units affect formatting only. Byte counts, sorting and explicit-unit search filters retain their original meaning. See [VALIDATION.md](VALIDATION.md) for automated and opt-in native checks.
