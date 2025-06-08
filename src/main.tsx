import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import TestResultsViewer from './App.tsx'
import "./index.css";

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <TestResultsViewer />
  </StrictMode>,
)
