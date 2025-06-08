# Playwright AI Test Reviewer

A modern React application that analyzes Playwright test trace files and provides AI-powered insights into test failures. Upload your Playwright trace ZIP files to get detailed analysis, visual comparisons, and actionable recommendations for fixing failed tests.

## Features

- 📁 **Single File Analysis**: Upload one ZIP file for individual test analysis
- 🔍 **Two-File Comparison**: Compare two trace files to identify differences
- 🤖 **AI-Powered Analysis**: Get intelligent insights into test failures using Google's Gemini AI
- 📸 **Screenshot Comparison**: Visual side-by-side comparison of passed vs failed test screenshots
- 🔧 **Raw Data Access**: View and copy detailed t race data for debugging
- 🎨 **Modern UI**: Clean, responsive interface built with React and Tailwind CSS

## Prerequisites

- Node.js (version 16 or higher)
- npm or yarn
- Google Gemini API key (for AI analysis features)

## Installation

1. Clone the repository:
```bash
cd playwrigth-ai-test-reviewer
```

2. Install dependencies:
```bash
npm install
```

3. Set up environment variables:
```bash
cp .env.example .env
```

4. Add your Google Gemini API key to the `.env` file:
```env
VITE_GEMINI_API_KEY=your_api_key_here
```

## Usage

### Development

Start the development server:
```bash
npm run dev
```

The application will be available at `http://localhost:5173`

### Production

Build the application:
```bash
npm run build
```

Preview the production build:
```bash
npm run preview
```


## Project Structure

```
playwrigth-ai-test-reviewer/
├── src/
│   ├── components/          # React components
│   │   ├── ui/             # Reusable UI components
│   │   ├── FileUploadZone.tsx
│   │   ├── TestResultItem.tsx
│   │   ├── ResultsSummary.tsx
│   │   └── LoadingSpinner.tsx
│   ├── hooks/              # Custom React hooks
│   │   └── useFileProcessing.ts
│   ├── types/              # TypeScript type definitions
│   │   └── index.ts
│   ├── App.tsx             # Main application component
│   └── main.tsx            # Application entry point
├── utils/                  # Utility functions
│   ├── zipProcessor.ts     # ZIP file processing logic
│   ├── utils.ts           # Helper functions
│   ├── types.ts           # Shared type definitions
│   ├── apiCall.ts         # AI API integration
│   └── index.ts           # Utility exports
├── public/                # Static assets
└── package.json           # Project dependencies
```

## Key Components

### [`zipProcessor.ts`](utils/zipProcessor.ts)
Handles ZIP file processing, trace analysis, and screenshot extraction from Playwright trace files.

### [`useFileProcessing.ts`](src/hooks/useFileProcessing.ts)
Custom React hook that manages file upload, processing state, and error handling.

### [`TestResultItem.tsx`](src/components/TestResultItem.tsx)
Component that displays individual test results with AI analysis, screenshots, and raw data.

### [`apiCall.ts`](utils/apiCall.ts)
Integrates with Google's Gemini AI to provide intelligent test failure analysis.

## API Integration

The application uses Google's Gemini AI for test analysis. The AI provides:

- **Confidence Level**: Assessment of analysis reliability
- **Error Cause**: Clear explanation of test failure reasons  
- **Fix Suggestion**: Actionable recommendations for resolving issues

## Tech Stack

- **Frontend**: React 19, TypeScript
- **Styling**: Tailwind CSS
- **Build Tool**: Vite
- **AI Integration**: Google Gemini AI via AI SDK
- **File Processing**: JSZip for handling trace files
- **Icons**: Lucide React
- **Markdown**: react-markdown for AI response rendering