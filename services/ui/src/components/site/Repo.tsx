import HeaderPage from "./pages/HeaderPage";
import { BENCHER_GITHUB_URL } from "./util";

const Repo = () => {
  window.location.href = BENCHER_GITHUB_URL;

  return (
    <HeaderPage
      page={{
        title: "GitHub Repo Redirect - Bencher",
        header: "Redirecting...",
        content: (
          <p>
            Redirecting to <a href={BENCHER_GITHUB_URL}>GitHub</a>.
          </p>
        ),
      }}
    />
  );
};

export default Repo;
