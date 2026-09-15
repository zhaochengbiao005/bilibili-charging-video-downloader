import React from 'react';
import { Info, Code, ShieldCheck, Heart } from 'lucide-react';

export function About() {
  return (
    <div className="max-w-4xl mx-auto w-full p-8 md:p-12 flex flex-col gap-10 min-h-full">
       <div className="text-center flex flex-col items-center mt-4 shrink-0">
          <div className="w-16 h-16 bg-gradient-to-tr from-bili-pink to-pink-300 rounded-2xl flex items-center justify-center text-white mb-6 shadow-md rotate-3">
             <Info size={32} strokeWidth={2.5} className="-rotate-3" />
          </div>
          <h1 className="text-4xl font-black tracking-tight mb-4 text-gray-900 drop-shadow-sm">About BiliDownloader</h1>
          <p className="text-lg text-gray-600 font-medium max-w-xl shadow-sm">
            A fast, secure, and modern tool to download your favorite Bilibili videos and audio tracks.
          </p>
       </div>

       <div className="grid grid-cols-1 md:grid-cols-2 gap-6 mt-4">
         <div className="glass-panel p-8 rounded-[2rem] flex flex-col gap-4">
            <div className="w-12 h-12 bg-blue-100 text-blue-600 rounded-xl flex items-center justify-center mb-2">
               <ShieldCheck size={24} strokeWidth={2.5} />
            </div>
            <h3 className="text-xl font-bold text-gray-900">Secure & Private</h3>
            <p className="text-gray-600 font-medium leading-relaxed">
               We don't store your history or personal data on our servers. All parsing and downloading happens directly between you and Bilibili.
            </p>
         </div>
         <div className="glass-panel p-8 rounded-[2rem] flex flex-col gap-4">
            <div className="w-12 h-12 bg-pink-100 text-bili-pink rounded-xl flex items-center justify-center mb-2">
               <Code size={24} strokeWidth={2.5} />
            </div>
            <h3 className="text-xl font-bold text-gray-900">Open Source Philosophy</h3>
            <p className="text-gray-600 font-medium leading-relaxed">
               Built with modern web technologies including React, Tailwind CSS, and Vite. Designed to be lightweight, fast, and beautiful.
            </p>
         </div>
       </div>

       <div className="glass-panel p-8 md:p-10 rounded-[2rem] mt-6 flex flex-col md:flex-row items-center justify-between gap-8">
          <div>
            <h3 className="text-2xl font-bold text-gray-900 mb-2">Support the project</h3>
            <p className="text-gray-600 font-medium">If you find this tool useful, consider supporting our development.</p>
          </div>
          <button className="flex items-center gap-2 px-8 py-4 bg-gradient-to-r from-bili-pink to-pink-400 text-white rounded-2xl font-bold shadow-lg shadow-pink-200 hover:scale-105 transition-all shrink-0">
             <Heart size={20} strokeWidth={2.5} />
             Donate Now
          </button>
       </div>

       <div className="text-center text-gray-500 font-medium text-sm mt-12 pb-8">
          <p>© {new Date().getFullYear()} BiliDownloader. All rights reserved.</p>
          <p className="mt-1">Version 2.4.1 Beta</p>
       </div>
    </div>
  );
}
