import React from 'react';

export function Mascot22({ className = '' }: { className?: string }) {
  return (
    <svg viewBox="0 0 96 78" className={className} role="img" aria-label="22">
      <path d="M26 70 32 19c2-10 11-16 21-14 10 2 18 11 18 22l-2 43H26Z" fill="#1d63dc" />
      <path d="M33 68 37 24c1-8 8-14 17-13 8 1 14 8 14 17l-2 40H33Z" fill="#f8f7ff" />
      <path d="M34 24c9 1 22-2 31-7 2 4 4 8 4 13l-1 7c-12-1-24-5-34-13Z" fill="#e5edff" />
      <path d="M38 15c6-8 22-8 29 2-10 2-19 3-29-2Z" fill="#fff" />
      <path d="M43 14c5-4 13-4 18 0" fill="none" stroke="#1d63dc" strokeWidth="3" strokeLinecap="round" />
      <text x="48" y="14" textAnchor="middle" fontSize="9" fontWeight="900" fill="#1d63dc">22</text>
      <circle cx="45" cy="39" r="2.8" fill="#273247" />
      <circle cx="59" cy="39" r="2.8" fill="#273247" />
      <path d="M46 50c4 4 10 4 14 0" fill="none" stroke="#ef6f99" strokeWidth="3" strokeLinecap="round" />
      <path d="M36 39c-5-2-8-1-11 2" fill="none" stroke="#273247" strokeWidth="3" strokeLinecap="round" />
      <path d="M68 39c5-2 8-1 11 2" fill="none" stroke="#273247" strokeWidth="3" strokeLinecap="round" />
    </svg>
  );
}

export function Mascot33({ className = '' }: { className?: string }) {
  return (
    <svg viewBox="0 0 96 78" className={className} role="img" aria-label="33">
      <path d="M26 70 30 28c1-14 11-23 24-23 14 0 25 11 27 25l5 40H26Z" fill="#ff6fa4" />
      <path d="M34 68 37 30c1-10 8-17 18-17s18 8 19 18l4 37H34Z" fill="#fff5f8" />
      <path d="M39 27c10 2 23 0 33-7 2 4 3 8 4 13-14 1-26-1-37-6Z" fill="#ffe1ec" />
      <circle cx="48" cy="41" r="2.8" fill="#773545" />
      <circle cx="62" cy="41" r="2.8" fill="#773545" />
      <path d="M50 52c4 4 10 4 14 0" fill="none" stroke="#ef6f99" strokeWidth="3" strokeLinecap="round" />
      <circle cx="35" cy="40" r="4" fill="#ffc1d6" />
      <circle cx="81" cy="40" r="4" fill="#ffc1d6" />
      <path d="M30 34c-5 0-8 4-8 8s3 7 7 8" fill="none" stroke="#ff6fa4" strokeWidth="4" strokeLinecap="round" />
      <path d="M82 34c5 0 8 4 8 8s-3 7-7 8" fill="none" stroke="#ff6fa4" strokeWidth="4" strokeLinecap="round" />
    </svg>
  );
}

export function AkariSilhouette({ className = '' }: { className?: string }) {
  return (
    <svg viewBox="0 0 72 72" className={className} role="img" aria-label="登录头像">
      <defs>
        <linearGradient id="akariBg" x1="12" y1="10" x2="60" y2="64" gradientUnits="userSpaceOnUse">
          <stop stopColor="#fff" />
          <stop offset="1" stopColor="#ffe7f0" />
        </linearGradient>
      </defs>
      <circle cx="36" cy="36" r="32" fill="url(#akariBg)" stroke="#ff7dad" strokeWidth="3" />
      <path d="M20 51c4-12 12-18 16-18s12 6 16 18H20Z" fill="#6b7280" opacity=".8" />
      <path d="M25 31c0-9 5-15 11-15s11 6 11 15c0 8-5 14-11 14s-11-6-11-14Z" fill="#4b5563" />
      <path d="M26 25c5-6 14-7 21-1-6 2-13 2-21 1Z" fill="#111827" opacity=".72" />
      <circle cx="32" cy="34" r="2" fill="#fff" opacity=".85" />
      <circle cx="41" cy="34" r="2" fill="#fff" opacity=".85" />
      <path d="M32 41c3 2 6 2 9 0" fill="none" stroke="#fff" strokeWidth="2" strokeLinecap="round" opacity=".8" />
    </svg>
  );
}

export function QrGlyph({ className = '' }: { className?: string }) {
  const cells = [
    [0, 0], [1, 0], [2, 0], [5, 0], [7, 0], [8, 0],
    [0, 1], [2, 1], [4, 1], [6, 1], [8, 1],
    [0, 2], [1, 2], [2, 2], [4, 2], [5, 2], [7, 2], [8, 2],
    [3, 3], [5, 3], [6, 3],
    [0, 4], [1, 4], [3, 4], [4, 4], [7, 4],
    [0, 5], [2, 5], [4, 5], [6, 5], [8, 5],
    [0, 6], [1, 6], [2, 6], [5, 6], [7, 6],
    [0, 7], [4, 7], [5, 7], [8, 7],
    [0, 8], [1, 8], [2, 8], [4, 8], [7, 8], [8, 8],
  ];

  return (
    <svg viewBox="0 0 180 180" className={className} role="img" aria-label="二维码">
      <rect x="1" y="1" width="178" height="178" rx="20" fill="#fff" stroke="#ff9fc0" strokeWidth="3" />
      {cells.map(([x, y]) => (
        <rect key={`${x}-${y}`} x={24 + x * 15} y={24 + y * 15} width="12" height="12" rx="1.5" fill="#1f2937" />
      ))}
      <rect x="67" y="67" width="46" height="46" rx="10" fill="#ff6fa4" />
      <rect x="78" y="78" width="24" height="24" rx="6" fill="#fff" />
      <circle cx="85" cy="90" r="2.5" fill="#ff6fa4" />
      <circle cx="96" cy="90" r="2.5" fill="#ff6fa4" />
      <path d="M84 97c4 3 9 3 13 0" fill="none" stroke="#ff6fa4" strokeWidth="2.5" strokeLinecap="round" />
      <path d="M19 41V21h20M141 21h20v20M19 139v20h20M161 139v20h-20" fill="none" stroke="#ff6fa4" strokeWidth="4" strokeLinecap="round" />
    </svg>
  );
}
