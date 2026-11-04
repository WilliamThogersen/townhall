import { useMemo, useState } from "react";
import { attachmentUrl } from "../api/attachments";
import type { Attachment } from "../api/types";
import { extensionTag, formatClock, formatSize } from "./format";
import { Lightbox, type LightboxImage } from "./Lightbox";

export interface ThumbsProps {
  readonly conversationId: string;
  readonly attachmentIds: readonly string[];
  readonly attachments: readonly Attachment[];
  readonly from?: string;
}

const MAX_IMAGES_IN_FULL = 3;
const SHOWN_WHEN_COLLAPSED = 2;

interface Opened {
  readonly index: number;
  readonly opener: HTMLElement;
}

type OnOpen = (index: number, opener: HTMLElement) => void;

export function lightboxImages(
  conversationId: string,
  images: readonly Attachment[],
  from = "you",
  win: object = window,
): readonly LightboxImage[] {
  return images.map((image) => ({
    src: attachmentUrl(conversationId, image.id, win),
    name: image.name,
    meta: [formatSize(image.size), `from ${from}`, formatClock(image.createdAt)].filter(Boolean).join(" · "),
  }));
}

function sharedIn(ids: readonly string[], attachments: readonly Attachment[]): readonly Attachment[] {
  const byId = new Map(attachments.map((attachment) => [attachment.id, attachment]));
  return ids.flatMap((id) => {
    const found = byId.get(id);
    return found ? [found] : [];
  });
}

interface RowProps {
  readonly conversationId: string;
  readonly items: readonly Attachment[];
}

function ImageRow({ conversationId, items, onOpen }: RowProps & { readonly onOpen: OnOpen }) {
  const shown = items.length > MAX_IMAGES_IN_FULL ? items.slice(0, SHOWN_WHEN_COLLAPSED) : items;
  const hidden = items.length - shown.length;
  return (
    <div className="th-msg-images">
      {shown.map((image, index) => (
        <button key={image.id} className="th-thumb" type="button" onClick={(event) => onOpen(index, event.currentTarget)}>
          <img src={attachmentUrl(conversationId, image.id)} alt={image.name} loading="lazy" decoding="async" />
        </button>
      ))}
      {hidden > 0 && (
        <button
          className="th-thumb th-thumb-more"
          type="button"
          aria-label={`Show ${hidden} more images`}
          onClick={(event) => onOpen(shown.length, event.currentTarget)}
        >
          +{hidden}
        </button>
      )}
    </div>
  );
}

function FileRow({ conversationId, items }: RowProps) {
  return (
    <div className="th-msg-files">
      {items.map((file) => (
        <a key={file.id} className="th-file" href={attachmentUrl(conversationId, file.id)} download={file.name}>
          <span className="th-file-ext">{extensionTag(file.name)}</span>
          <span className="th-file-name">{file.name}</span>
          <span className="th-file-size">{formatSize(file.size)}</span>
        </a>
      ))}
    </div>
  );
}

export function Thumbs({ conversationId, attachmentIds, attachments, from = "you" }: ThumbsProps) {
  const [opened, setOpened] = useState<Opened | null>(null);
  const shared = useMemo(() => sharedIn(attachmentIds, attachments), [attachmentIds, attachments]);
  const images = shared.filter((attachment) => attachment.kind === "image");
  const files = shared.filter((attachment) => attachment.kind === "file");
  if (shared.length === 0) return null;
  return (
    <>
      {images.length > 0 && (
        <ImageRow
          conversationId={conversationId}
          items={images}
          onOpen={(index, opener) => setOpened({ index, opener })}
        />
      )}
      {files.length > 0 && <FileRow conversationId={conversationId} items={files} />}
      {opened && (
        <Lightbox
          images={lightboxImages(conversationId, images, from)}
          startIndex={opened.index}
          returnFocusTo={opened.opener}
          onClose={() => setOpened(null)}
        />
      )}
    </>
  );
}
