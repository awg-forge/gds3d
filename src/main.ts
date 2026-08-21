import { mount } from "svelte";
import App from "./App.svelte";
import "./styles.css";
import "./svelte.css";

const target = document.getElementById("app");

if (!target) throw new Error("app mount target is missing");

mount(App, { target });
