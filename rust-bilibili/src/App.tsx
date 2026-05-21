import React, { useEffect, useState } from 'react';
import { HashRouter, useLocation, useNavigate } from 'react-router-dom';
import { Sidebar } from './components/Sidebar';
import { Home } from './pages/Home';
import { History } from './pages/History';
import { Settings } from './pages/Settings';
import { About } from './pages/About';
import { LoginModal } from './components/LoginModal';
import type { LoginStatus } from './types';
import * as Bridge from './bridge';

function Root() {
  const location = useLocation();
  const navigate = useNavigate();
  const [isLoginOpen, setIsLoginOpen] = useState(false);
  const [loginStatus, setLoginStatus] = useState<LoginStatus | null>(null);

  const modalRoute = location.pathname === '/history'
    ? { title: '下载历史', content: <History /> }
    : location.pathname === '/settings'
      ? { title: '设置', content: <Settings /> }
      : location.pathname === '/about'
        ? { title: '关于', content: <About /> }
        : null;
  const hasOverlay = Boolean(modalRoute) || isLoginOpen;

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && isLoginOpen) {
        setIsLoginOpen(false);
        return;
      }
      if (e.key === 'Escape' && modalRoute) {
        navigate('/');
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [isLoginOpen, modalRoute, navigate]);

  useEffect(() => {
    Bridge.checkLogin().then(setLoginStatus).catch((err) => {
      setLoginStatus({
        is_login: false,
        message: err instanceof Error ? err.message : '登录状态检查失败',
      });
    });
  }, []);

  return (
    <div className="flex w-screen h-screen bg-sky-50/45 backdrop-blur-[32px] overflow-hidden text-gray-800 font-sans selection:bg-bili-pink selection:text-white relative">

      {/* Background layer */}
      <div
        className={`flex flex-1 w-full transition-all duration-[400ms] ease-[cubic-bezier(0.16,1,0.3,1)] overflow-hidden ${
          hasOverlay ? 'blur-[8px] scale-[0.97] opacity-50 pointer-events-none' : ''
        }`}
      >
        <Sidebar loginStatus={loginStatus} onLoginClick={() => setIsLoginOpen(true)} />
        <main className="flex-1 flex flex-col overflow-y-auto custom-scrollbar relative z-10">
          <Home />
        </main>
      </div>

      <div
        className={`fixed inset-0 z-50 flex items-center justify-center p-3 sm:p-5 transition-all duration-[400ms] ease-[cubic-bezier(0.16,1,0.3,1)] ${
          isLoginOpen ? 'opacity-100 pointer-events-auto' : 'opacity-0 pointer-events-none'
        }`}
        onClick={() => setIsLoginOpen(false)}
      >
        <div
          className={`w-full max-w-[448px] max-h-[calc(100vh-1.5rem)] transition-all duration-[400ms] ease-[cubic-bezier(0.16,1,0.3,1)] ${
            isLoginOpen ? 'translate-y-0 scale-100' : 'translate-y-8 scale-95'
          }`}
          onClick={(e) => e.stopPropagation()}
        >
          <LoginModal
            loginStatus={loginStatus}
            onLoginChange={setLoginStatus}
            onClose={() => setIsLoginOpen(false)}
          />
        </div>
      </div>

      <div
        className={`fixed inset-0 z-50 flex items-center justify-center p-4 sm:p-8 transition-all duration-[400ms] ease-[cubic-bezier(0.16,1,0.3,1)] ${
          modalRoute ? 'opacity-100 pointer-events-auto' : 'opacity-0 pointer-events-none'
        }`}
        onClick={() => navigate('/')}
      >
        <div
          className={`relative w-full max-w-[900px] max-h-[88vh] bg-white/86 backdrop-blur-[32px] border border-white rounded-[2.5rem] shadow-[0_24px_80px_rgba(31,38,135,0.18)] flex flex-col overflow-hidden transition-all duration-[400ms] ease-[cubic-bezier(0.16,1,0.3,1)] ${
            modalRoute ? 'translate-y-0 scale-100' : 'translate-y-8 scale-95'
          }`}
          onClick={(e) => e.stopPropagation()}
          aria-label={modalRoute?.title ?? '弹窗'}
        >
          <button
            onClick={() => navigate('/')}
            className="absolute top-6 right-6 w-10 h-10 bg-white/60 hover:bg-white text-gray-500 hover:text-bili-pink hover:scale-110 rounded-full flex items-center justify-center shadow-sm border border-white transition-all z-20"
            aria-label="关闭"
          >
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round"><line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" /></svg>
          </button>
          <div className="w-full flex-1 overflow-y-auto custom-scrollbar relative min-h-[50vh]">
            {modalRoute?.content}
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
