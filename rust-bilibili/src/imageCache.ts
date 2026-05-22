import * as Bridge from './bridge';

const MAX_CACHE_SIZE = 160;
const cache = new Map<string, string>();

function remember(url: string, src: string) {
  if (!url.trim()) return;
  if (cache.has(url)) cache.delete(url);
  cache.set(url, src);
  while (cache.size > MAX_CACHE_SIZE) {
    const oldest = cache.keys().next().value;
    if (!oldest) break;
    cache.delete(oldest);
  }
}

export function getCachedImageDataUrl(url: string): string | undefined {
  return cache.get(url);
}

export async function getImageDataUrl(url: string): Promise<string> {
  const normalized = url.trim();
  if (!normalized) return '';
  const cached = cache.get(normalized);
  if (cached) return cached;
  remember(normalized, normalized);
  try {
    const src = await Bridge.fetchImageDataUrl(normalized);
    remember(normalized, src || normalized);
    return src || normalized;
  } catch {
    remember(normalized, normalized);
    return normalized;
  }
}

export function prefetchImageDataUrls(urls: Array<string | null | undefined>, limit = 6) {
  [...new Set(urls.filter((url): url is string => Boolean(url?.trim())))]
    .filter((url) => !cache.has(url))
    .slice(0, limit)
    .forEach((url) => {
      void getImageDataUrl(url);
    });
}
