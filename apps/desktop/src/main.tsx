import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import ChatApp from "./ChatApp";
import { currentWindowLabel } from "./lib/api";
import "./app.css";

const windowLabel = currentWindowLabel();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    {windowLabel === "chat" ? <ChatApp /> : <App />}
  </React.StrictMode>,
);
