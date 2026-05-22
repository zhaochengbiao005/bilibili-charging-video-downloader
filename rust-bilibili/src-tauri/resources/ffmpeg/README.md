# Bundled FFmpeg

`npm run prepare:ffmpeg` copies the build-time FFmpeg binary into this directory.
`npm run tauri:build` runs that preparation step automatically before bundling, so
the Windows installer can ship an out-of-the-box MP4/MP3 experience.

Runtime lookup order:

1. Bundled resource: `ffmpeg/ffmpeg.exe`
2. Application directory cache: `<exe-dir>/ffmpeg/ffmpeg.exe`
3. System `PATH`: `ffmpeg`

The binary and copied notice files are intentionally ignored by git. If you
distribute a build with FFmpeg included, include the corresponding FFmpeg license
notice with the release.
