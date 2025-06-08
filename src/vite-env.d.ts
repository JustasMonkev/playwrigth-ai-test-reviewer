/// <reference types="vite/client" />
/// <reference types="react" />
/// <reference types="react-dom" />

declare global {
  namespace JSX {
    interface IntrinsicElements {
      [elemName: string]: any;
    }
  }
}

interface ImportMetaEnv {
    readonly VITE_GEMINI_API_KEY: string
}

interface ImportMeta {
    readonly env: ImportMetaEnv
}
