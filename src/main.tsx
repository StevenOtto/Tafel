import React from "react";
import ReactDOM from "react-dom/client";
import { ControlApp } from "./app/ControlApp";
import "./styles.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ControlApp />
  </React.StrictMode>,
);
