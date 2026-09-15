import React, { useEffect } from 'react';
import { Navigate, Route, HashRouter, Routes, useLocation, useNavigate } from 'react-router-dom';
import { Sidebar } from './components/Sidebar';
import { Home } from './pages/Home';
import { History } from './pages/History';
import { Settings } from './pages/Settings';
import { About } from './pages/About';

function Root() {
  const location = useLocation();
  const navigate = useNavigate();

  const isModalOpen = location.pathname === '/about';
  const isAbout = location.pathname === '/about';

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && isModalOpen) {
        navigate('/');
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [isModalOpen, navigate]);

  return (
    <div className="flex w-full h-full sm:w-[calc(100vw-2rem)] sm:h-[calc(100vh-2rem)] bg-sky-50/40 backdrop-blur-[32px] sm:rounded-[2rem] overflow-hidden text-gray-800 font-sans selection:bg-bili-pink selection:text-white relative border border-white/40 shadow-[0_16px_64px_-12px_rgba(0,0,0,0.2)]">

      {/* Background layer */}
      <div
        className={`flex flex-1 w-full transition-all duration-[400ms] ease-[cubic-bezier(0.16,1,0.3,1)] overflow-hidden ${
          isAbout ? 'blur-[8px] scale-[0.97] opacity-50 pointer-events-none' : ''
        }`}
      >
        <Sidebar />
        <main className="flex-1 flex flex-col overflow-y-auto custom-scrollbar relative z-10">
          <Routes>
            <Route path="/" element={<Home />} />
            <Route path="/history" element={<History />} />
            <Route path="/settings" element={<Settings />} />
            <Route path="*" element={<Navigate to="/" replace />} />
          </Routes>
        </main>
      </div>

      {/* About modal overlay */}
      <div
        className={`fixed inset-0 z-50 flex items-center justify-center p-4 sm:p-8 transition-all duration-[400ms] ease-[cubic-bezier(0.16,1,0.3,1)] ${
          isAbout ? 'opacity-100 pointer-events-auto' : 'opacity-0 pointer-events-none'
        }`}
        onClick={() => navigate('/')}
      >
        <div
          className={`relative w-full max-w-4xl max-h-[90vh] bg-white/80 backdrop-blur-[32px] border border-white rounded-[2.5rem] shadow-[0_20px_60px_-15px_rgba(251,114,153,0.15)] flex flex-col overflow-hidden transition-all duration-[400ms] ease-[cubic-bezier(0.16,1,0.3,1)] ${
            isAbout ? 'translate-y-0 scale-100' : 'translate-y-8 scale-95'
          }`}
          onClick={(e) => e.stopPropagation()}
        >
          <button
            onClick={() => navigate('/')}
            className="absolute top-6 right-6 w-10 h-10 bg-white/60 hover:bg-white text-gray-500 hover:text-bili-pink hover:scale-110 rounded-full flex items-center justify-center shadow-sm border border-white transition-all z-20"
          >
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round"><line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" /></svg>
          </button>
          <div className="w-full flex-1 overflow-y-auto custom-scrollbar relative min-h-[50vh]">
            <Routes>
              <Route path="/about" element={<About />} />
            </Routes>
          </div>
        </div>
      </div>
    </div>
  );
}

export default function App() {
  return (
    <HashRouter>
      <Root />
    </HashRouter>
  );
}
