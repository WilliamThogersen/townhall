import {
  useCallback,
  useLayoutEffect,
  useRef,
  useState,
  type KeyboardEvent,
  type MouseEvent,
  type RefObject,
  type SyntheticEvent,
} from "react";
import { toMessage } from "../api/errors";
import { Button } from "../ui/Button";
import { Kbd } from "../ui/Kbd";
import { trapTab } from "./focusTrap";
import { copyImage, downloadFile } from "./imageActions";

export interface LightboxImage {
  readonly src: string;
  readonly name: string;
  readonly meta?: string;
  readonly alt?: string;
}

export interface LightboxProps {
  readonly images: readonly LightboxImage[];
  readonly startIndex?: number;
  readonly onClose: () => void;
  readonly returnFocusTo?: HTMLElement | null;
  readonly label?: string;
}

interface Notice {
  readonly src: string;
  readonly copied: boolean;
  readonly error: string | null;
}

interface PixelSize {
  readonly src: string;
  readonly text: string;
}

const clamp = (value: number, max: number): number => Math.min(Math.max(value, 0), max);

function useModalDialog(
  dialogRef: RefObject<HTMLDialogElement | null>,
  focusRef: RefObject<HTMLButtonElement | null>,
  returnFocusTo: HTMLElement | null,
): void {
  const returnTo = useRef(returnFocusTo);
  useLayoutEffect(() => {
    const dialog = dialogRef.current;
    if (dialog === null) return undefined;
    const previous = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    if (typeof dialog.showModal !== "function") dialog.setAttribute("open", "");
    else if (!dialog.open) dialog.showModal();
    focusRef.current?.focus();
    return () => {
      if (typeof dialog.close === "function") dialog.close();
      (returnTo.current ?? previous)?.focus();
    };
  }, [dialogRef, focusRef]);
}

function useImageActions(image: LightboxImage) {
  const [notice, setNotice] = useState<Notice | null>(null);
  const { src, name } = image;
  const run = useCallback(
    async (verb: "copy" | "download") => {
      try {
        await (verb === "copy" ? copyImage(src) : downloadFile(src, name));
        setNotice({ src, copied: verb === "copy", error: null });
      } catch (error) {
        setNotice({ src, copied: false, error: `error: cannot ${verb} ${name}: ${toMessage(error)}` });
      }
    },
    [src, name],
  );
  const current = notice?.src === src ? notice : null;
  return {
    copy: () => void run("copy"),
    download: () => void run("download"),
    copied: current?.copied ?? false,
    error: current?.error ?? null,
  };
}

interface KeyActions {
  readonly step: (delta: number) => void;
  readonly close: () => void;
  readonly copy: () => void;
}

function handleKey(event: KeyboardEvent<HTMLDialogElement>, actions: KeyActions): void {
  const plain = !event.altKey && !event.ctrlKey && !event.metaKey;
  event.stopPropagation();
  if (event.key === "Tab") trapTab(event, event.currentTarget);
  else if (event.key === "Escape") {
    event.preventDefault();
    actions.close();
  } else if (event.key === "ArrowLeft" || event.key === "ArrowRight") {
    event.preventDefault();
    actions.step(event.key === "ArrowLeft" ? -1 : 1);
  } else if (event.key === "c" && plain) actions.copy();
}

function closesOnClick(event: MouseEvent<HTMLDialogElement>): boolean {
  const target = event.target;
  if (target === event.currentTarget) return true;
  return target instanceof HTMLElement && target.classList.contains("th-lightbox-stage");
}

type ImageActions = ReturnType<typeof useImageActions>;

interface BarProps {
  readonly image: LightboxImage;
  readonly meta: string;
  readonly position: string;
  readonly actions: ImageActions;
  readonly closeRef: RefObject<HTMLButtonElement | null>;
  readonly onClose: () => void;
}

function LightboxBar({ image, meta, position, actions, closeRef, onClose }: BarProps) {
  return (
    <div className="th-lightbox-bar">
      <span className="th-lightbox-name">{image.name}</span>
      <span className="th-lightbox-meta">{meta}</span>
      <div className="th-lightbox-actions">
        {actions.error && (
          <span className="th-field-error" role="alert">
            {actions.error}
          </span>
        )}
        <span className="th-lightbox-meta">{position}</span>
        <Button variant="ghost" size="sm" onClick={actions.copy}>
          {actions.copied ? "copied" : "copy"}
        </Button>
        <Button variant="ghost" size="sm" onClick={actions.download}>
          download
        </Button>
        <Button variant="ghost" size="sm" onClick={onClose} ref={closeRef}>
          close <Kbd>esc</Kbd>
        </Button>
      </div>
    </div>
  );
}

interface StageProps {
  readonly image: LightboxImage;
  readonly atStart: boolean;
  readonly atEnd: boolean;
  readonly step: (delta: number) => void;
  readonly onLoad: (event: SyntheticEvent<HTMLImageElement>) => void;
}

function LightboxStage({ image, atStart, atEnd, step, onLoad }: StageProps) {
  return (
    <div className="th-lightbox-stage">
      <img src={image.src} alt={image.alt ?? image.name} onLoad={onLoad} />
      <button
        className="th-lightbox-nav"
        type="button"
        data-dir="prev"
        aria-label="Previous image"
        disabled={atStart}
        onClick={() => step(-1)}
      >
        ‹
      </button>
      <button
        className="th-lightbox-nav"
        type="button"
        data-dir="next"
        aria-label="Next image"
        disabled={atEnd}
        onClick={() => step(1)}
      >
        ›
      </button>
    </div>
  );
}

interface FootProps {
  readonly images: readonly LightboxImage[];
  readonly index: number;
  readonly pick: (index: number) => void;
}

function LightboxFoot({ images, index, pick }: FootProps) {
  return (
    <div className="th-lightbox-foot">
      <div className="th-lightbox-strip">
        {images.map((item, n) => (
          <button
            key={`${n}-${item.src}`}
            className="th-thumb"
            type="button"
            aria-label={`Show ${item.name}`}
            aria-current={n === index ? "true" : undefined}
            onClick={() => pick(n)}
          >
            <img src={item.src} alt="" />
          </button>
        ))}
      </div>
      <span>
        <Kbd>←</Kbd> <Kbd>→</Kbd> step · <Kbd>c</Kbd> copy · <Kbd>esc</Kbd> close
      </span>
    </div>
  );
}

function usePixelSize(src: string) {
  const [pixels, setPixels] = useState<PixelSize | null>(null);
  const onLoad = (event: SyntheticEvent<HTMLImageElement>) => {
    const { naturalWidth, naturalHeight } = event.currentTarget;
    if (naturalWidth > 0) setPixels({ src, text: `${naturalWidth} × ${naturalHeight}` });
  };
  return { text: pixels?.src === src ? pixels.text : "", onLoad };
}

function LightboxDialog({ images, startIndex = 0, onClose, returnFocusTo = null, label = "Image viewer" }: LightboxProps) {
  const dialogRef = useRef<HTMLDialogElement>(null);
  const closeRef = useRef<HTMLButtonElement>(null);
  const last = images.length - 1;
  const [index, setIndex] = useState(() => clamp(startIndex, last));
  const image = images[clamp(index, last)] as LightboxImage;
  const actions = useImageActions(image);
  const pixels = usePixelSize(image.src);
  useModalDialog(dialogRef, closeRef, returnFocusTo);
  const step = (delta: number) => setIndex((current) => clamp(current + delta, last));
  const meta = [pixels.text, image.meta ?? ""].filter(Boolean).join(" · ");
  return (
    <dialog
      ref={dialogRef}
      className="th-lightbox"
      role="dialog"
      aria-modal="true"
      aria-label={label}
      onKeyDown={(event) => handleKey(event, { step, close: onClose, copy: actions.copy })}
      onCancel={(event) => {
        event.preventDefault();
        onClose();
      }}
      onClick={(event) => closesOnClick(event) && onClose()}
    >
      <LightboxBar
        image={image}
        meta={meta}
        position={`${index + 1} / ${images.length}`}
        actions={actions}
        closeRef={closeRef}
        onClose={onClose}
      />
      <LightboxStage image={image} atStart={index === 0} atEnd={index === last} step={step} onLoad={pixels.onLoad} />
      <LightboxFoot images={images} index={index} pick={setIndex} />
    </dialog>
  );
}

export function Lightbox(props: LightboxProps) {
  if (props.images.length === 0) return null;
  return <LightboxDialog {...props} />;
}
