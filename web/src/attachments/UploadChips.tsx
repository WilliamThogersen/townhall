import { useState, type CSSProperties } from "react";
import { FileCardBody } from "./FileCardBody";
import { formatSize } from "./format";
import { Lightbox } from "./Lightbox";
import type { PendingUpload } from "./uploads";
import "./chips.css";

export interface UploadChipsProps {
  readonly items: readonly PendingUpload[];
  readonly onRemove: (key: string) => void;
}

interface ChipProps {
  readonly upload: PendingUpload;
  readonly onRemove: (key: string) => void;
}

type ChipState = "uploading" | "error" | undefined;

function chipState(upload: PendingUpload): ChipState {
  if (upload.status === "uploading") return "uploading";
  return upload.status === "failed" ? "error" : undefined;
}

const failureText = (upload: PendingUpload): string => `error: ${upload.error ?? "the upload failed"}`;

function RemoveButton({ upload, onRemove }: ChipProps) {
  const name = upload.file.name;
  const label = upload.status === "uploading" ? `Cancel upload of ${name}` : `Remove ${name}`;
  return (
    <button type="button" className="th-thumb-remove" aria-label={label} onClick={() => onRemove(upload.key)}>
      ×
    </button>
  );
}

function FailedImageChip({ upload, onRemove }: ChipProps) {
  const reason = failureText(upload);
  return (
    <button
      type="button"
      className="th-thumb"
      data-state="error"
      title={reason}
      aria-label={`Remove ${upload.file.name}, ${reason}`}
      onClick={() => onRemove(upload.key)}
    >
      {upload.previewUrl && <img src={upload.previewUrl} alt="" />}
    </button>
  );
}

function ImageChip({ upload, onRemove }: ChipProps) {
  const [opener, setOpener] = useState<HTMLElement | null>(null);
  if (upload.status === "failed") return <FailedImageChip upload={upload} onRemove={onRemove} />;
  const uploading = upload.status === "uploading";
  const name = upload.file.name;
  const style = uploading ? ({ "--progress": `${upload.percent}%` } as CSSProperties) : undefined;
  const preview = upload.previewUrl;
  const alt = uploading ? `${name}, uploading ${upload.percent}%` : name;
  return (
    <div className="th-thumb" data-state={chipState(upload)} style={style}>
      {preview && !uploading && (
        <button
          type="button"
          className="th-thumb-open"
          aria-label={`View ${name}`}
          onClick={(event) => setOpener(event.currentTarget)}
        >
          <img src={preview} alt={alt} />
        </button>
      )}
      {preview && uploading && <img src={preview} alt={alt} />}
      <RemoveButton upload={upload} onRemove={onRemove} />
      {opener && (
        <Lightbox
          images={[{ src: upload.previewUrl ?? "", name, meta: formatSize(upload.file.size) }]}
          returnFocusTo={opener}
          onClose={() => setOpener(null)}
        />
      )}
    </div>
  );
}

function FileChip({ upload, onRemove }: ChipProps) {
  const name = upload.file.name;
  const size = upload.status === "uploading" ? `${upload.percent}%` : formatSize(upload.file.size);
  return (
    <span
      className="th-file"
      data-state={chipState(upload)}
      title={upload.status === "failed" ? failureText(upload) : undefined}
    >
      <FileCardBody name={name} detail={size} />
      <RemoveButton upload={upload} onRemove={onRemove} />
    </span>
  );
}

export function UploadChips({ items, onRemove }: UploadChipsProps) {
  if (items.length === 0) return null;
  return (
    <div className="th-composer-attachments" role="group" aria-label="Attachments">
      {items.map((upload) =>
        upload.isImage ? (
          <ImageChip key={upload.key} upload={upload} onRemove={onRemove} />
        ) : (
          <FileChip key={upload.key} upload={upload} onRemove={onRemove} />
        ),
      )}
    </div>
  );
}
