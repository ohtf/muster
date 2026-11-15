// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-PanicButton-app-store-exception

// One page, four windows: `?view=` picks what this window shows.

import { mount } from "svelte";
import "./app.css";
import AlertView from "./views/AlertView.svelte";
import ButtonView from "./views/ButtonView.svelte";
import MainView from "./views/MainView.svelte";
import PanelView from "./views/PanelView.svelte";

const views = { button: ButtonView, panel: PanelView, alert: AlertView, main: MainView };
const name = new URLSearchParams(location.search).get("view") ?? "main";
const View = views[name as keyof typeof views] ?? MainView;
// Per-window page background (app.css), since component styles are global.
document.documentElement.dataset.view = name in views ? name : "main";
const target = document.getElementById("app");
if (target) mount(View, { target });
