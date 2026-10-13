import { useEffect, useState } from "react";
import { petPreview } from "./api";

const objectUrls = new Map<string, string>();
const pending = new Map<string, Promise<string>>();

function loadPreview(
  path: string,
  frameWidth: number,
  frameHeight: number,
): Promise<string> {
  const key = `${path}|${frameWidth}x${frameHeight}`;
  const cached = objectUrls.get(key);
  if (cached) return Promise.resolve(cached);
  const current = pending.get(key);
  if (current) return current;

  const request = petPreview(path, frameWidth, frameHeight)
    .then((bytes) => {
      const url = URL.createObjectURL(
        new Blob([new Uint8Array(bytes)], { type: "image/png" }),
      );
      objectUrls.set(key, url);
      return url;
    })
    .finally(() => pending.delete(key));
  pending.set(key, request);
  return request;
}

export function usePetPreview(
  path: string | undefined,
  frameWidth: number,
  frameHeight: number,
): string {
  const [url, setUrl] = useState("");

  useEffect(() => {
    if (!path || !frameWidth || !frameHeight) {
      setUrl("");
      return;
    }
    let active = true;
    setUrl(objectUrls.get(`${path}|${frameWidth}x${frameHeight}`) ?? "");
    loadPreview(path, frameWidth, frameHeight)
      .then((next) => {
        if (active) setUrl(next);
      })
      .catch(() => {
        if (active) setUrl("");
      });
    return () => {
      active = false;
    };
  }, [path, frameWidth, frameHeight]);

  return url;
}
