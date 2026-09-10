/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_API_BASE: string;
  readonly VITE_OSGB_PREVIEW_URL: string;
  readonly VITE_TILES_PREVIEW_URL: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
