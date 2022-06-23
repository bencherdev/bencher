import "./styles/styles.scss";

import { Component } from "solid-js";
import { Routes, Route } from "solid-app-router";

import { Navbar } from "./components/site/Navbar";
import { GoogleAnalytics } from "./components/site/GoogleAnalytics";
import { AuthFormPage } from "./components/auth/AuthFormPage";
import { LandingPage } from "./components/LandingPage";

const App: Component = () => {
  return (
    <>
      <GoogleAnalytics />
      <Navbar />
      <Routes>
        <Route path="/" element={<LandingPage />} />
        <Route path="/auth/signup" element={<AuthFormPage kind="signup" />} />
        <Route path="/auth/login" element={<AuthFormPage kind="login" />} />
      </Routes>
    </>
  );
};

export default App;
