import { useEffect, useMemo, useRef, useState, type DragEvent } from "react";

export interface FileDropProps {
  readonly onDragEnter: (event: DragEvent) => void;
  readonly onDragOver: (event: DragEvent) => void;
  readonly onDragLeave: (event: DragEvent) => void;
  readonly onDrop: (event: DragEvent) => void;
}

export interface FileDrop {
  readonly dragging: boolean;
  readonly dropProps: FileDropProps;
}

function carriesFiles(event: DragEvent): boolean {
  return Array.from(event.dataTransfer?.types ?? []).includes("Files");
}

export function useFileDrop(onFiles: (files: readonly File[]) => void, enabled = true): FileDrop {
  const [dragging, setDragging] = useState(false);
  const depth = useRef(0);
  const latestOnFiles = useRef(onFiles);
  useEffect(() => {
    latestOnFiles.current = onFiles;
  }, [onFiles]);

  const dropProps = useMemo<FileDropProps>(() => {
    const allow = (event: DragEvent) => {
      if (!enabled) return;
      event.preventDefault();
      event.dataTransfer.dropEffect = "copy";
    };
    return {
      onDragEnter: (event) => {
        if (!carriesFiles(event)) return;
        depth.current += 1;
        setDragging(true);
        allow(event);
      },
      onDragOver: (event) => {
        if (carriesFiles(event)) allow(event);
      },
      onDragLeave: (event) => {
        if (!carriesFiles(event)) return;
        depth.current = Math.max(0, depth.current - 1);
        if (depth.current === 0) setDragging(false);
      },
      onDrop: (event) => {
        if (!carriesFiles(event)) return;
        depth.current = 0;
        setDragging(false);
        if (!enabled) return;
        event.preventDefault();
        const files = Array.from(event.dataTransfer.files);
        if (files.length > 0) latestOnFiles.current(files);
      },
    };
  }, [enabled]);

  return { dragging: enabled && dragging, dropProps };
}
