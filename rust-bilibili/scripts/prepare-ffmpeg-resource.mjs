import { copyFile, mkdir, stat } from 'node:fs/promises';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import ffmpegPath from 'ffmpeg-static';

const rootDir = fileURLToPath(new URL('..', import.meta.url));
const resourceDir = join(rootDir, 'src-tauri', 'resources', 'ffmpeg');
const bundledPath = join(resourceDir, 'ffmpeg.exe');
const sourcePath = process.env.FFMPEG_BIN || ffmpegPath;

async function exists(path) {
  try {
    await stat(path);
    return true;
  } catch {
    return false;
  }
}

await mkdir(resourceDir, { recursive: true });

if (!sourcePath || !(await exists(sourcePath))) {
  throw new Error('No FFmpeg binary found. Run npm install and retry, or set FFMPEG_BIN.');
}

await copyFile(sourcePath, bundledPath);
for (const suffix of ['.LICENSE', '.README']) {
  const sourceNotice = `${sourcePath}${suffix}`;
  if (await exists(sourceNotice)) {
    await copyFile(sourceNotice, `${bundledPath}${suffix}`);
  }
}

console.log(`Bundled FFmpeg ready: ${bundledPath}`);
