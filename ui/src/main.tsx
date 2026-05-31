import React from "react";
import ReactDOM from "react-dom/client";
import "./styles/globals.css";
import App from "./App";
import { ThemeProvider } from "@/components/theme/ThemeProvider";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <ThemeProvider>
      <App />
    </ThemeProvider>
  </React.StrictMode>,
);
