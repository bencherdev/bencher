import { Route, Navigate } from "solid-app-router";
import HeaderPage from "../site/pages/HeaderPage";
import termsPage from "./config/termsPage";
import privacyPage from "./config/privacyPage";
import licensePage from "./config/licensePage";
import legalPage from "./config/legalPage";

const LegalRoutes = (props) => {
  const headerPage = (page) => {
    return <HeaderPage page={page} />;
  };

  return (
    <>
      {/* Legal Routes */}
      <Route path="/" element={headerPage(legalPage)} />
      <Route path="/terms-of-use" element={headerPage(termsPage)} />
      <Route path="/privacy" element={headerPage(privacyPage)} />
      <Route path="/license" element={headerPage(licensePage)} />
    </>
  );
};

export default LegalRoutes;
