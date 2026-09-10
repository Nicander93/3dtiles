/**
 * @deprecated Prefer `api/desktop` — kept for Phase 1 import compatibility.
 * Pages should not call invoke() directly.
 */
export {
  isTauri,
  desktop,
  api as desktopApi,
} from '../api/desktop';

import { desktop } from '../api/desktop';

export const selectInputDirectory = () => desktop.selectInputDirectory();
export const selectOutputDirectory = () => desktop.selectOutputDirectory();
export const selectTilesetFile = () => desktop.selectTilesetFile();
