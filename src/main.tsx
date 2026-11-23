// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import './styles/index.css';
import { App } from './App';

const fx = new URLSearchParams(location.search).get('fx');
try {
  const saved = sessionStorage.getItem('inputlab.fx');
  document.documentElement.dataset.fx = fx ?? saved ?? 'full';
} catch {
  document.documentElement.dataset.fx = fx ?? 'full';
}

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
