import { usePetPreview } from "../lib/usePetPreview";

export function PetThumb({
  path,
  frameWidth,
  frameHeight,
  fallback,
  size = "normal",
}: {
  path?: string;
  frameWidth: number;
  frameHeight: number;
  fallback: string;
  size?: "normal" | "large";
}) {
  const url = usePetPreview(path, frameWidth, frameHeight);
  return (
    <div className={`pet-thumb pet-thumb-${size}`} aria-hidden>
      {url ? <img src={url} alt="" /> : <span>{fallback.slice(0, 1).toUpperCase()}</span>}
    </div>
  );
}
