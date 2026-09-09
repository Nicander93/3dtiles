# GeoForge 3D Web UI (v0.1.0)

本地 · 高效 · 开放 — Vite + React + TypeScript Chinese UI.

## Requirements

- Node.js 18+ (20 recommended)
- Local API at http://127.0.0.1:8787 (VITE_API_BASE)
- Optional: serve examples/preview on 8080; Vite proxies /preview

## Run

```bash
cd apps/web
npm install
npm run dev
```

Open http://127.0.0.1:5173

## Build

```bash
cd apps/web
npm install
npm run build
npm run preview
```

## Env

See .env / .env.example

- VITE_API_BASE=http://127.0.0.1:8787
- VITE_OSGB_PREVIEW_URL=/preview/osgb.html
- VITE_TILES_PREVIEW_URL=/preview/index.html

## API

- GET /api/health
- POST /api/osgb/scan {path}
- POST /api/tasks {operation, input, output, options}
- GET /api/tasks
- GET /api/tasks/:id
- POST /api/tasks/:id/cancel

When API is down: Chinese empty/error states. No fake conversion progress.

## Routes

- / 工作区
- /osgb/convert OSGB转换
- /processing 正在处理
- /preview/osgb OSGB预览
- /preview/tiles 3D Tiles预览
- /history 处理记录
- /results 处理成果
- /settings 设置与帮助

## Key files

- src/App.tsx
- src/layouts/AppLayout.tsx
- src/api/client.ts
- src/pages/*
- src/styles/global.css
