const PNG = "image/png";
const REVOKE_AFTER_MS = 1000;

async function fetchBlob(url: string): Promise<Blob> {
  const response = await fetch(url);
  if (!response.ok) throw new Error(`HTTP ${response.status}`);
  return response.blob();
}

function canvasToPng(canvas: HTMLCanvasElement): Promise<Blob> {
  return new Promise((resolve, reject) => {
    canvas.toBlob((png) => {
      if (png === null) reject(new Error("the image could not be turned into a png"));
      else resolve(png);
    }, PNG);
  });
}

async function toPng(blob: Blob): Promise<Blob> {
  if (blob.type === PNG) return blob;
  const bitmap = await createImageBitmap(blob);
  const canvas = document.createElement("canvas");
  canvas.width = bitmap.width;
  canvas.height = bitmap.height;
  canvas.getContext("2d")?.drawImage(bitmap, 0, 0);
  return canvasToPng(canvas);
}

function canCopyImages(): boolean {
  return typeof ClipboardItem !== "undefined" && typeof navigator.clipboard?.write === "function";
}

export async function copyImage(url: string): Promise<void> {
  if (!canCopyImages()) throw new Error("this window cannot copy images");
  const item = new ClipboardItem({ [PNG]: fetchBlob(url).then(toPng) });
  await navigator.clipboard.write([item]);
}

export async function downloadFile(url: string, name: string): Promise<void> {
  const blob = await fetchBlob(url);
  const href = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = href;
  link.download = name;
  link.hidden = true;
  document.body.append(link);
  link.click();
  link.remove();
  setTimeout(() => URL.revokeObjectURL(href), REVOKE_AFTER_MS);
}
