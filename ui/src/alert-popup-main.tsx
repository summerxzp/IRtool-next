import React from "react";
import ReactDOM from "react-dom/client";
import AlertPopupWindow from "./components/AlertPopupWindow";

// Sync theme from main window's localStorage
try {
  const theme = localStorage.getItem("irtool-theme");
  let resolved: "light" | "dark";
  if (theme === "light" || theme === "dark") {
    resolved = theme;
  } else {
    resolved = window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
  }
  document.documentElement.setAttribute("data-theme", resolved);
  if (resolved === "dark") {
    document.documentElement.classList.add("dark");
  } else {
    document.documentElement.classList.remove("dark");
  }
} catch {}

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <AlertPopupWindow />
  </React.StrictMode>,
);
