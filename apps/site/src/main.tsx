import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "@games/ui/ui.css";
import "./styles/app.css";
import { App } from "./App.tsx";
import { AuthProvider } from "./lib/auth.tsx";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <AuthProvider>
      <App />
    </AuthProvider>
  </StrictMode>,
);
