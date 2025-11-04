import {defineConfig} from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from "@tailwindcss/vite";

export default defineConfig({
    plugins: [react(), tailwindcss()],
    build: {
        rollupOptions: {
            output: {
                manualChunks: {
                    // Split React and React DOM into separate chunk
                    'react-vendor': ['react', 'react-dom', 'react/jsx-runtime'],

                    // Split AI/Markdown libraries (heavy dependencies)
                    'ai-vendor': [
                        '@google/generative-ai',
                        '@ai-sdk/google',
                        'ai',
                        'react-markdown'
                    ],

                    // Split UI libraries
                    'ui-vendor': [
                        'lucide-react',
                        'class-variance-authority',
                        'clsx',
                        'tailwind-merge'
                    ],

                    // Split ZIP processing library
                    'zip-vendor': ['jszip'],
                },
            },
        },
        // Increase chunk size warning limit to 1000kb
        chunkSizeWarningLimit: 1000,
    },
})
