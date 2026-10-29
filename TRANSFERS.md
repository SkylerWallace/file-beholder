# File operations and Undo

Copy/Cut/Paste and cross-panel drops use the same background queue. Ctrl+A selects visible items; Ctrl+C/X/V copy, cut and paste. Cut and deletion ask for confirmation. Delete uses the Windows Recycle Bin; Shift+Delete permanently deletes after confirmation. File drops ask for Copy & Paste, Cut & Paste or Cancel before queuing anything.

## Queue and conflicts

The queue starts collapsed. Its header shows unified progress, status, transfer rate, completed bytes/items and ETA; expanding shows individual jobs. Existing destinations offer Replace, Skip or Keep Both. Failed files get up to four attempts before processing continues. Cancellation retains completed work and cleans up incomplete staging files where possible.

Choose a completed job's “View failed / skipped files” button to open a large popup with every affected path and reason. Compact rows are rendered only as needed while scrolling, and the filter searches paths and reasons. Hover a path or reason for full details. Each row offers Retry and Location. Copy and move file retries use the original destination folder and ask about existing files again. Retry All queues all retryable failed and skipped files from that job. Successful files are not retried. Details remain available in the session's most recent 30 jobs; an open report is retained while retry jobs are queued.

Sources and the destination are snapshotted for a drag/drop request. Dropping onto a folder row targets that panel's current directory, not the hovered folder. Drops back into the source panel or outside a destination cancel. Reparse points are not recursively followed.

Transfers preserve bytes and modified timestamps, but do not promise to preserve every attribute, alternate stream, hard-link identity, ACL, encryption or compression setting. They are not whole-directory atomic transactions. Sizes are logical file lengths. Binary/Metric unit settings apply to progress and rates as well as file rows.

Optional Windows notification and completion sound settings are disabled by default. They use the native Windows desktop services and fire once after the full batch completes, with aggregate successful/failed counts. Cancelled batches stay silent. Operating-system notification/audio preferences still apply.

## Open and Properties

Double-click a folder or choose Open folder to navigate. Settings → Allow opening files controls whether double-click, Open file, or Open file with can launch a file. Turn it off to avoid accidental access to user data while continuing to browse folders and transfer files. File-opening actions appear dimmed and disabled when it is off. Existing saved `open_files` preferences are preserved.

With file opening allowed, Open file invokes its registered app or shows the native Windows app chooser if no association exists. Open file with always shows the native chooser; cancelling it is silent. Copy, Cut, Paste and Delete appear in a horizontal action strip above the context menu's opening actions. Environment expansion applies to path-box navigation, not shell command execution.

Properties uses the native Windows Shell sheet, including combined same-folder selections when supported. A persistent COM/message-loop worker keeps the app responsive. If unavailable, a read-only fallback displays cached metadata and partial sizes. Reveal in File tree expands the selected directory manually.

Drag a folder from the File tree or a file panel onto any panel's tab bar to open it as a new tab at the drop position. This also works within the same panel and preserves shadow-copy identity when dragging within a snapshot. Dropping from a file panel into another file listing continues to offer a transfer instead.

## Undo

The ribbon Undo action and Ctrl+Z outside text inputs reverse the latest completed job. Undo waits until pending transfers finish or are cancelled. Completed portions of cancelled jobs may be undoable. It removes unchanged copies and newly created empty directories, moves files back, restores replacement backups and restores recoverable Recycle Bin items. Permanent deletion creates a non-undoable history barrier.

Undo verifies content and refuses to remove an edited copy, overwrite an occupied original, or remove a nonempty copied directory. Completed inverse actions are checkpointed so a blocked Undo can resume. Affected tabs refresh automatically, including when journal paths use Windows verbatim prefixes or UNC aliases.

History and replacement backups live under `%LOCALAPPDATA%/FileBeholder/history`, with a temporary-directory fallback. They survive normal restarts; pending queues do not. Backups consume disk space and have no automatic retention limit. A replacement is refused if its backup cannot be prepared. Keep this directory when its backups are needed.

Incomplete jobs after a crash are not offered for automatic Undo. A crash between a filesystem change and its journal checkpoint may need manual recovery; a crashed Undo may leave `undo.lock`. If a recycled item cannot be matched to a Bin record, use Windows Recycle Bin to restore it manually. See [VALIDATION.md](VALIDATION.md) for test coverage.

## Shadow copies and row colors

The ribbon's Shadow copies button lists existing Windows VSS snapshots across local drives, with the original drive and local date/time. Choose one to open its root in a new tab in the panel selected when the picker was opened. Refresh queries Windows again. Loading, empty, and access-error states are shown in the picker; querying or reading snapshots may require Run as admin.

Snapshot tabs use compact labels such as `VSS C: 2026-10-24`, with the full timestamp and path on hover. The label stays visible while browsing within that snapshot, and the snapshot identity survives tab moves and remembered sessions. Snapshot folders use recursive size scanning rather than the live volume's MFT. Existing snapshots are accessed directly through their [Windows VSS device paths](https://learn.microsoft.com/en-us/windows/win32/vss/requestor-access-to-shadow-copied-data); opening one does not create a snapshot or a mount point.

Settings → Alternate file row colors is enabled by default. Alternation follows the visible, sorted/filtered row order in both themes; selection and hover colors take priority. The choice is saved with other preferences.
