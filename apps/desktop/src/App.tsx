import { Navigate, Route, Routes } from "react-router-dom";
import { AppLayout } from "./layouts/AppLayout";
import { Workspace } from "./pages/Workspace";
import { OsgbConvert } from "./pages/OsgbConvert";
import { Processing } from "./pages/Processing";
import { TilesPreview } from "./pages/TilesPreview";
import { Results } from "./pages/Results";
import { Settings } from "./pages/Settings";
import { ProcessTiles } from "./pages/ProcessTiles";
import { ModelConvert } from "./pages/ModelConvert";

export default function App() {
  return (
    <Routes>
      <Route element={<AppLayout />}>
        <Route index element={<Workspace />} />
        <Route path="osgb/convert" element={<OsgbConvert />} />
        <Route path="model/convert" element={<ModelConvert />} />
        <Route path="processing" element={<Processing />} />
        <Route path="preview/tiles" element={<TilesPreview />} />
        <Route path="history" element={<Navigate to="/processing" replace />} />
        <Route path="results" element={<Results />} />
        <Route path="tiles/process" element={<ProcessTiles />} />
        <Route path="settings" element={<Settings />} />
      </Route>
    </Routes>
  );
}
