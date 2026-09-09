import { Route, Routes } from "react-router-dom";
import { AppLayout } from "./layouts/AppLayout";
import { Workspace } from "./pages/Workspace";
import { OsgbConvert } from "./pages/OsgbConvert";
import { Processing } from "./pages/Processing";
import { OsgbPreview } from "./pages/OsgbPreview";
import { TilesPreview } from "./pages/TilesPreview";
import { History } from "./pages/History";
import { Results } from "./pages/Results";
import { Settings } from "./pages/Settings";
import { ProcessTiles } from "./pages/ProcessTiles";

export default function App() {
  return (
    <Routes>
      <Route element={<AppLayout />}>
        <Route index element={<Workspace />} />
        <Route path="osgb/convert" element={<OsgbConvert />} />
        <Route path="processing" element={<Processing />} />
        <Route path="preview/osgb" element={<OsgbPreview />} />
        <Route path="preview/tiles" element={<TilesPreview />} />
        <Route path="history" element={<History />} />
        <Route path="results" element={<Results />} />
        <Route path="tiles/process" element={<ProcessTiles />} />
        <Route path="settings" element={<Settings />} />
      </Route>
    </Routes>
  );
}
