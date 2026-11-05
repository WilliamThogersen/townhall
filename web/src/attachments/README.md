# attachments

Files and images in a conversation, styled by `styles/attachments.css` (preview `#attachments`). Everything is exported from `index.ts`.

## api (`../api/attachments.ts`)

- `uploadAttachment(conversationId, file, onProgress?, signal?)`: one multipart `file` per request over XMLHttpRequest (fetch has no upload progress). `onProgress(percent)` gets whole percents. Resolves the `Attachment`; rejects an `ApiError` (`http` with the daemon's words such as `shot.png is over 20 MB`, `network`, `protocol`) or an `AbortError`.
- `listAttachments(conversationId, signal?)`, `attachmentUrl(conversationId, id)` (the desktop base inside Tauri), `MAX_ATTACHMENT_BYTES`.

## Composer side

- `useUploads(conversationId)`: `items` (`PendingUpload`: `status` `uploading` with `percent`, `done` with `attachment`, `failed` with `error` and `retryable`), `uploading`, `attachmentIds` (done only), `error` (last failure, no `error:` prefix), `canRetry`, `add(files)`, `retry(key)`, `retryFailed()`, `remove(key)` (cancels a running upload), `clear()`. Checks each file first: `<name> is over 20 MB`, `<name> is empty`, `<name> is not an image, text, code or pdf` (those fail for good, no request). A dropped connection or a 5xx can be retried; a 4xx cannot. A new `conversationId` or unmounting cancels everything. The composer owns one; you rarely need it directly.
- `UploadChips({ items, onRemove })`: `.th-composer-attachments`. Images as 64 by 48 thumbnails (a finished one opens the lightbox on click; progress line while uploading, red border on failure, click a failed one to remove it), files as `.th-file` chips (`60%` in place of the size while uploading).

## Conversation page

- `useFileDrop(onFiles, enabled = true)` returns `{ dragging, dropProps }`. Spread `dropProps` on the whole conversation area, render `<DropZone />` while `dragging`, hand the files to the composer:

  ```tsx
  const composer = useRef<ComposerHandle>(null);
  const drop = useFileDrop((files) => composer.current?.addFiles(files));
  <section className="th-convo-view" {...drop.dropProps}>
    {drop.dragging && <DropZone />}
    ...
    <Composer ref={composer} conversationId={id} onSubmit={send} />
  </section>
  ```

  `.th-drop` is `position: absolute`, so the area needs `position: relative` (`.th-convo-view` has none in the reference CSS).
- `Thumbs({ conversationId, attachmentIds, attachments, from = "you", collapse = true })`: put it in a `you` entry's `.th-msg-body` after the text, with the entry's `attachments` and the conversation's `Attachment[]` (`ConversationDetail.attachments`). Images become `.th-msg-images` thumbnails (more than 3 collapse to 2 and `+N`), files `.th-msg-files` chips linking to `attachmentUrl` with `download`. Unknown ids are skipped. `collapse={false}` shows every image, for the session panel's `files` grid. A thumbnail opens the lightbox on that message's images.
- `Lightbox({ images, startIndex, onClose, returnFocusTo, label = "Image viewer" })`: mount it to open, unmount to close. A native `<dialog>` (`showModal()` where available), `aria-modal`, focus starts on `close` and stays inside (tab wraps), `← →` step, `esc`, `close` or a click beside the image calls `onClose`, `c` and `copy` put the image on the clipboard as png, `download` saves it. Focus goes back to `returnFocusTo` (or what had focus) on close. `lightboxImages(conversationId, attachments, from)` builds its `images` from attachments; the bar adds the pixel size once the image loads.
